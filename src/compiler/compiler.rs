use std::fmt::{Display, Formatter};

use rlox_gc::Gc;

use crate::{
    common::{OpCode, Value},
    compiler::scanner::{Scanner, Token, TokenType},
    object::{ObjFunction, ObjString},
    vm::{
        Chunk, LoxHeap,
        vm::{GlobalIndices, VMError},
    },
};

#[derive(PartialEq, PartialOrd)]
#[allow(dead_code)]
enum Precedence {
    None,
    Assignment,
    Or,
    And,
    Equality,
    Comparison,
    Term,
    Factor,
    Unary,
    Call,
    Primary,
}

type PrefixFn<'src> = fn(&mut Compiler<'src>);
type InfixFn<'src> = PrefixFn<'src>;

struct ParseRule<'src> {
    prefix: Option<PrefixFn<'src>>,
    infix: Option<InfixFn<'src>>,
    precedence: Precedence,
}

#[derive(Debug, Copy, Clone)]
struct Local<'src> {
    token: Token<'src>,
    depth: u8,
    is_captured: bool,
}

impl<'src> Default for Local<'src> {
    fn default() -> Self {
        Self {
            token: Default::default(),
            depth: u8::MAX,
            is_captured: false,
        }
    }
}

#[derive(Debug)]
pub struct CompileError {
    pub message: String,
    pub line: usize,
    pub location: ErrorLocation,
}

#[derive(Debug)]
pub enum ErrorLocation {
    EndOfFile,
    Lexeme(String),
    Unknown,
}

impl Display for CompileError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let location = match &self.location {
            ErrorLocation::EndOfFile => " at end".to_string(),
            ErrorLocation::Lexeme(s) => format!(" at '{}'", s),
            ErrorLocation::Unknown => String::new(),
        };
        write!(
            f,
            "[line {}] Error{}: {}",
            self.line, location, self.message
        )
    }
}

#[derive(Debug, Clone, Copy)]
struct Upvalue {
    pub index: u8,
    pub is_local: bool,
}

#[derive(Debug)]
struct FunctionContext<'src> {
    pub function: ObjFunction,
    pub locals: Vec<Local<'src>>,
    pub upvalues: Vec<Upvalue>,
    pub scope_depth: u8,
}

impl<'src> FunctionContext<'src> {
    pub fn new(name: Option<Gc<ObjString>>) -> Self {
        Self {
            function: ObjFunction::new(name),
            locals: Vec::with_capacity(u8::MAX as usize),
            upvalues: Vec::new(),
            scope_depth: 0,
        }
    }
}

#[derive(Debug)]
pub struct Compiler<'src> {
    contexts: Vec<FunctionContext<'src>>,
    scanner: Scanner<'src>,
    previous: Token<'src>,
    current: Token<'src>,
    heap: &'src mut LoxHeap,
    global_indices: &'src mut GlobalIndices,
    errors: Vec<CompileError>,
    can_assign: bool,
    had_error: bool,
    panic_mode: bool,
}

impl<'src> Compiler<'src> {
    pub fn compile(
        source: &'src str,
        heap: &mut LoxHeap,
        global_indices: &mut GlobalIndices,
    ) -> Result<ObjFunction, VMError> {
        let mut compiler = Compiler {
            contexts: vec![FunctionContext::new(None)],
            scanner: Scanner::new(source),
            previous: Token::default(),
            current: Token::default(),
            heap: heap,
            global_indices,
            errors: Vec::new(),
            can_assign: false,
            had_error: false,
            panic_mode: false,
        };
        // reserve slot 0 for function itself
        compiler.current_context_mut().locals.push(Local {
            token: Token::default(),
            depth: 0,
            is_captured: false,
        });

        compiler.advance();
        while !compiler.matches(TokenType::EOF) {
            compiler.declaration();
        }
        let func = compiler.end_compiler();

        if compiler.had_error {
            return Err(VMError::CompileError(compiler.errors));
        }

        Ok(func)
    }

    fn advance(&mut self) {
        self.previous = std::mem::replace(&mut self.current, self.scanner.scan_token());

        while self.current.kind == TokenType::Error {
            self.error_at_current(&format!("unexpected token {}", self.current.lexeme));
            self.current = self.scanner.scan_token();
        }
    }

    fn consume(&mut self, kind: TokenType, message: &str) {
        if self.current.kind == kind {
            self.advance();
        } else {
            self.error_at_current(message);
        }
    }

    fn check(&self, kind: TokenType) -> bool {
        self.current.kind == kind
    }

    fn matches(&mut self, kind: TokenType) -> bool {
        if !self.check(kind) {
            return false;
        }
        self.advance();
        true
    }

    fn end_compiler(&mut self) -> ObjFunction {
        self.emit_return();
        #[cfg(debug_assertions)]
        {
            if !self.had_error {
                self.current_chunk_mut().disassemble("script");
            }
        }
        self.contexts.pop().unwrap().function
    }

    fn end_function(&mut self) -> FunctionContext<'_> {
        self.emit_return();
        #[cfg(debug_assertions)]
        {
            if !self.had_error {
                if let Some(name) = self.current_context().function.name {
                    self.current_chunk_mut().disassemble(&name);
                } else {
                    self.current_chunk_mut().disassemble("script");
                }
            }
        }
        self.contexts.pop().unwrap()
    }

    fn begin_function(&mut self) {
        let name = self.heap.intern(self.previous.lexeme);
        self.contexts.push(FunctionContext::new(Some(name)));
        // reserve slot 0 for function itself
        self.current_context_mut().locals.push(Local {
            token: Token::default(),
            depth: 0,
            is_captured: false,
        });
    }

    fn begin_scope(&mut self) {
        self.current_context_mut().scope_depth += 1;
    }

    fn end_scope(&mut self) {
        self.current_context_mut().scope_depth -= 1;

        let scope_depth = self.current_context().scope_depth;

        while self
            .current_context()
            .locals
            .last()
            .map_or(false, |l| l.depth > scope_depth)
        {
            if self.current_context().locals.last().unwrap().is_captured {
                self.emit_byte(OpCode::CloseUpvalue);
            } else {
                self.emit_byte(OpCode::Pop);
            }
            self.current_context_mut().locals.pop();
        }
    }

    fn emit_byte(&mut self, byte: impl Into<u8>) {
        let line = self.previous.line;
        self.current_chunk_mut().write_byte(byte.into(), line);
    }

    fn emit_bytes(&mut self, byte1: impl Into<u8>, byte2: impl Into<u8>) {
        self.emit_byte(byte1);
        self.emit_byte(byte2);
    }

    fn emit_loop(&mut self, loop_start: usize) {
        self.emit_byte(OpCode::Loop);

        let offset = self.current_chunk_mut().code.len() - loop_start + 2;
        if offset > u16::MAX as usize {
            self.error_at_current("Loop body too large.");
        }

        self.emit_byte(((offset >> 8) & 0xff) as u8);
        self.emit_byte((offset & 0xff) as u8);
    }

    fn emit_jump(&mut self, opcode: OpCode) -> usize {
        self.emit_byte(opcode);
        self.emit_byte(0xff);
        self.emit_byte(0xff);
        self.current_chunk_mut().code.len() - 2
    }

    fn patch_jump(&mut self, offset: usize) {
        let jump = self.current_chunk_mut().code.len() - offset - 2;
        if jump > u16::MAX as usize {
            self.error_at_current("Too much code to jump over.");
        }

        self.current_chunk_mut().code[offset] = ((jump >> 8) & 0xff) as u8;
        self.current_chunk_mut().code[offset + 1] = (jump & 0xff) as u8;
    }

    fn emit_const(&mut self, value: Value) {
        let index = self.make_constant(value);
        self.emit_byte(OpCode::Constant);
        self.emit_byte(index);
    }

    fn make_constant(&mut self, value: Value) -> u8 {
        let idx = self.current_chunk_mut().add_constant(value);
        if idx > u8::MAX {
            self.error_at_current("Too many constants in one chunk.");
            return 0;
        }
        idx as u8
    }

    fn emit_return(&mut self) {
        self.emit_byte(OpCode::Nil);
        self.emit_byte(OpCode::Return);
    }

    fn expression(&mut self) {
        self.parse_precedence(Precedence::Assignment)
    }

    fn block(&mut self) {
        while !self.check(TokenType::RightBrace) && !self.check(TokenType::EOF) {
            self.declaration();
        }

        self.consume(TokenType::RightBrace, "Expected '}' after block.");
    }

    fn resolve_local(&mut self, name: &str) -> Option<u8> {
        for (i, local) in self.current_context_mut().locals.iter().enumerate().rev() {
            if local.token.lexeme == name {
                if local.depth == u8::MAX {
                    self.error_at_current("Can't read local variable in its own initializer.");
                }
                return Some(i as u8);
            }
        }
        None
    }

    fn resolve_local_in(name: &str, functions: &[FunctionContext]) -> Option<u8> {
        let locals = &functions.last()?.locals;
        for (i, local) in locals.iter().enumerate().rev() {
            if local.token.lexeme == name {
                if local.depth == u8::MAX {
                    return None;
                }
                return Some(i as u8);
            }
        }
        None
    }

    fn resolve_upvalue(&mut self, name: &str) -> Option<u8> {
        Self::resolve_upvalue_in(name, &mut self.contexts)
    }

    fn resolve_upvalue_in(name: &str, function_states: &mut [FunctionContext]) -> Option<u8> {
        if function_states.len() < 2 {
            return None;
        }

        let (current, enclosing) = function_states.split_last_mut().unwrap();

        if let Some(local_slot) = Self::resolve_local_in(name, enclosing) {
            enclosing.last_mut().unwrap().locals[local_slot as usize].is_captured = true;
            return Some(Self::add_upvalue(current, local_slot, true));
        }

        if let Some(upvalue_slot) = Self::resolve_upvalue_in(name, enclosing) {
            return Some(Self::add_upvalue(current, upvalue_slot, false));
        }

        None
    }

    fn add_upvalue(function_state: &mut FunctionContext, index: u8, is_local: bool) -> u8 {
        if let Some(i) = function_state
            .upvalues
            .iter()
            .position(|uv| uv.index == index && uv.is_local == is_local)
        {
            return i as u8;
        }

        let count = function_state.upvalues.len() as u8;
        // if count >= u8::MAX {
        //     return Err("Too many closure variables in function.");
        // }
        function_state.upvalues.push(Upvalue { index, is_local });
        function_state.function.upvalue_count = count + 1;
        count
    }

    fn global_slot(&mut self, name: &str) -> u8 {
        let str_ref = self.heap.intern(name);
        if let Some(slot) = self.global_indices.get(&str_ref) {
            return *slot;
        }
        let slot = self.global_indices.len() as u8;
        self.global_indices.insert(str_ref, slot);
        slot
    }

    fn add_local(&mut self, token: Token<'src>) {
        if self.current_context_mut().locals.len() == u8::MAX as usize {
            self.error_at_current("Too many variables in function.");
            return;
        }
        self.current_context_mut().locals.push(Local {
            token,
            depth: u8::MAX,
            is_captured: false,
        });
    }

    fn declare_variable(&mut self) {
        if self.current_context_mut().scope_depth == 0 {
            return;
        }
        let token = self.previous;
        let current_depth = self.current_context().scope_depth;
        let duplicate = self
            .current_context()
            .locals
            .iter()
            .rev()
            .take_while(|l| l.depth >= current_depth)
            .any(|l| l.token.lexeme == token.lexeme);
        if duplicate {
            self.error_at_current("Already a variable with this name in this scope.");
        }
        self.add_local(token);
    }

    fn parse_variable(&mut self, message: &str) -> u8 {
        self.consume(TokenType::Identifier, message);
        self.declare_variable();
        if self.current_context_mut().scope_depth > 0 {
            return 0;
        }
        self.global_slot(self.previous.lexeme)
    }

    fn define_variable(&mut self, var: u8) {
        if self.current_context_mut().scope_depth > 0 {
            // mark initialized
            self.current_context_mut().locals.last_mut().unwrap().depth =
                self.current_context_mut().scope_depth;
            return;
        }
        self.emit_bytes(OpCode::DefineGlobal, var);
    }

    fn argument_list(&mut self) -> u8 {
        let mut arg_count = 0;
        if !self.check(TokenType::RightParen) {
            loop {
                self.expression();
                if arg_count == 255 {
                    self.error_at_current("Can't have more than 255 arguments.");
                }
                arg_count += 1;
                if !self.matches(TokenType::Comma) {
                    break;
                }
            }
        }
        self.consume(TokenType::RightParen, "Expected ')' after arguments.");
        arg_count
    }

    fn var_declaration(&mut self) {
        let var = self.parse_variable("Expected variable name.");
        if self.matches(TokenType::Equal) {
            self.expression();
        } else {
            self.emit_byte(OpCode::Nil);
        }

        self.consume(
            TokenType::Semicolon,
            "Expected ';' after variable declaration.",
        );

        self.define_variable(var);
    }

    fn function_declaration(&mut self) {
        let global = self.parse_variable("Expected function name.");
        self.mark_initialized();
        self.function();
        self.define_variable(global);
    }

    fn expression_statement(&mut self) {
        self.expression();
        self.consume(TokenType::Semicolon, "Expected ';' after expression.");
        self.emit_byte(OpCode::Pop);
    }

    fn switch_statement(&mut self) {
        self.consume(TokenType::LeftParen, "Expected '(' after switch.");
        self.expression();
        self.consume(TokenType::RightParen, "Expected ')' after expression.");
        self.consume(TokenType::LeftBrace, "Expected '{' after switch statement.");

        let mut exit_jumps = Vec::with_capacity(u8::MAX as usize);

        while !self.check(TokenType::RightBrace) {
            if self.matches(TokenType::Case) {
                exit_jumps.push(self.switch_case());
            } else if self.matches(TokenType::Default) {
                exit_jumps.push(self.switch_default());
            }
        }

        exit_jumps
            .iter()
            .for_each(|offset| self.patch_jump(*offset));

        self.emit_byte(OpCode::Pop);
        self.consume(TokenType::RightBrace, "Expected '}' after switch cases.");
    }

    fn switch_case(&mut self) -> usize {
        self.expression();
        self.consume(TokenType::Colon, "Expected ':' after case.");
        self.emit_byte(OpCode::SwitchEq);

        let next_case = self.emit_jump(OpCode::JumpIfFalse);
        self.emit_byte(OpCode::Pop);

        while !self.check(TokenType::Case)
            && !self.check(TokenType::Default)
            && !self.check(TokenType::RightBrace)
            && !self.check(TokenType::EOF)
        {
            self.statement();
        }

        let exit_jump = self.emit_jump(OpCode::Jump);
        self.patch_jump(next_case);
        self.emit_byte(OpCode::Pop);

        exit_jump
    }

    fn switch_default(&mut self) -> usize {
        self.consume(TokenType::Colon, "Expected ':' after default case.");

        while !self.check(TokenType::Case)
            && !self.check(TokenType::Default)
            && !self.check(TokenType::RightBrace)
            && !self.check(TokenType::EOF)
        {
            self.statement();
        }
        self.emit_jump(OpCode::Jump)
    }

    fn if_statement(&mut self) {
        self.consume(TokenType::LeftParen, "Expected '(' after 'if'.");
        self.expression();
        self.consume(TokenType::RightParen, "Expected ')' after condition.");

        let then_jump = self.emit_jump(OpCode::JumpIfFalse);
        self.emit_byte(OpCode::Pop);

        self.statement();

        let else_jump = self.emit_jump(OpCode::Jump);

        self.patch_jump(then_jump);
        self.emit_byte(OpCode::Pop);

        if self.matches(TokenType::Else) {
            self.statement();
        }
        self.patch_jump(else_jump);
    }

    fn while_statement(&mut self) {
        let loop_start = self.current_chunk_mut().code.len();
        self.consume(TokenType::LeftParen, "Expected '(' after 'while'.");
        self.expression();
        self.consume(TokenType::RightParen, "Expected ')' after 'if'.");

        let exit_jump = self.emit_jump(OpCode::JumpIfFalse);
        self.emit_byte(OpCode::Pop);
        self.statement();
        self.emit_loop(loop_start);

        self.patch_jump(exit_jump);
        self.emit_byte(OpCode::Pop);
    }

    fn for_statement(&mut self) {
        self.begin_scope();
        self.consume(TokenType::LeftParen, "Expected '(' after 'for'.");
        if self.matches(TokenType::Var) {
            self.var_declaration();
        } else if self.matches(TokenType::Semicolon) {
            // do nothing
        } else {
            self.expression_statement();
        }

        let mut loop_start = self.current_chunk_mut().code.len();
        let mut exit_jump: Option<usize> = None;

        if !self.matches(TokenType::Semicolon) {
            self.expression();
            self.consume(TokenType::Semicolon, "Expected ';' after loop condition.");

            exit_jump = Some(self.emit_jump(OpCode::JumpIfFalse));
            self.emit_byte(OpCode::Pop);
        }

        if !self.matches(TokenType::RightParen) {
            let body_jump = self.emit_jump(OpCode::Jump);
            let increment_start = self.current_chunk_mut().code.len();
            self.expression();
            self.emit_byte(OpCode::Pop);
            self.consume(TokenType::RightParen, "Expected ')' after 'for' clauses.");

            self.emit_loop(loop_start);
            loop_start = increment_start;
            self.patch_jump(body_jump);
        }

        self.statement();
        self.emit_loop(loop_start);

        if let Some(exit_jump) = exit_jump {
            self.patch_jump(exit_jump);
            self.emit_byte(OpCode::Pop);
        }
        self.end_scope();
    }

    fn and(&mut self) {
        let end_jump = self.emit_jump(OpCode::JumpIfFalse);
        self.emit_byte(OpCode::Pop);
        self.parse_precedence(Precedence::And);
        self.patch_jump(end_jump);
    }

    fn or(&mut self) {
        let else_jump = self.emit_jump(OpCode::JumpIfFalse);
        let end_jump = self.emit_jump(OpCode::Jump);

        self.patch_jump(else_jump);
        self.emit_byte(OpCode::Pop);

        self.parse_precedence(Precedence::Or);
        self.patch_jump(end_jump);
    }

    fn print_statement(&mut self) {
        self.expression();
        self.consume(TokenType::Semicolon, "Expected ';' after value.");
        self.emit_byte(OpCode::Print);
    }

    fn return_statement(&mut self) {
        if self.contexts.is_empty() {
            self.error_at_current("Can't return from top-level code.");
        }
        if self.matches(TokenType::Semicolon) {
            self.emit_return();
        } else {
            self.expression();
            self.consume(TokenType::Semicolon, "Expected ';' after return value.");
            self.emit_byte(OpCode::Return);
        }
    }

    fn synchronize(&mut self) {
        self.panic_mode = false;
        while self.current.kind != TokenType::EOF {
            if self.previous.kind == TokenType::Semicolon {
                return;
            }
            match self.current.kind {
                TokenType::Class
                | TokenType::Fun
                | TokenType::Var
                | TokenType::For
                | TokenType::If
                | TokenType::While
                | TokenType::Print
                | TokenType::Return => return,
                _ => {}
            }
            self.advance();
        }
    }

    fn declaration(&mut self) {
        if self.matches(TokenType::Class) {
            self.class_declaration();
        } else if self.matches(TokenType::Fun) {
            self.function_declaration();
        } else if self.matches(TokenType::Var) {
            self.var_declaration();
        } else {
            self.statement();
        }

        if self.panic_mode {
            self.synchronize();
        }
    }

    fn statement(&mut self) {
        if self.matches(TokenType::Print) {
            self.print_statement();
        } else if self.matches(TokenType::LeftBrace) {
            self.begin_scope();
            self.block();
            self.end_scope();
        } else if self.matches(TokenType::If) {
            self.if_statement();
        } else if self.matches(TokenType::Return) {
            self.return_statement();
        } else if self.matches(TokenType::Switch) {
            self.switch_statement();
        } else if self.matches(TokenType::While) {
            self.while_statement();
        } else if self.matches(TokenType::For) {
            self.for_statement();
        } else {
            self.expression_statement();
        }
    }

    fn grouping(&mut self) {
        self.expression();
        self.consume(TokenType::RightParen, "Expected ')' after expression.");
    }

    fn number(&mut self) {
        let number: f64 = self.previous.lexeme.parse().unwrap();
        let value = Value::Number(number);

        self.emit_const(value);
    }

    fn literal(&mut self) {
        match self.previous.kind {
            TokenType::False => self.emit_byte(OpCode::False),
            TokenType::True => self.emit_byte(OpCode::True),
            TokenType::Nil => self.emit_byte(OpCode::Nil),
            _ => unreachable!(),
        }
    }

    fn unary(&mut self) {
        let kind = self.previous.kind;

        self.parse_precedence(Precedence::Assignment);

        match kind {
            TokenType::Minus => self.emit_byte(OpCode::Negate),
            TokenType::Bang => self.emit_byte(OpCode::Not),
            _ => unreachable!(),
        }
    }

    fn binary(&mut self) {
        let kind = self.previous.kind;
        let rule = Compiler::get_rule(kind);
        self.parse_precedence(rule.precedence);

        match kind {
            TokenType::Plus => self.emit_byte(OpCode::Add),
            TokenType::Minus => self.emit_byte(OpCode::Subtract),
            TokenType::Star => self.emit_byte(OpCode::Multiply),
            TokenType::Slash => self.emit_byte(OpCode::Divide),
            TokenType::BangEqual => self.emit_bytes(OpCode::Equal, OpCode::Not),
            TokenType::EqualEqual => self.emit_byte(OpCode::Equal),
            TokenType::Greater => self.emit_byte(OpCode::Greater),
            TokenType::GreaterEqual => self.emit_bytes(OpCode::Less, OpCode::Not),
            TokenType::Less => self.emit_byte(OpCode::Less),
            TokenType::LessEqual => self.emit_bytes(OpCode::Greater, OpCode::Not),
            _ => unreachable!(),
        }
    }

    fn call(&mut self) {
        let arg_count = self.argument_list();
        self.emit_bytes(OpCode::Call, arg_count);
    }

    fn dot(&mut self) {
        self.consume(TokenType::Identifier, "Expected property name after '.'.");
        let name = self.heap.intern(&self.previous.lexeme);
        let name_index = self.make_constant(Value::String(name));

        if self.can_assign && self.matches(TokenType::Equal) {
            self.expression();
            self.emit_bytes(OpCode::SetProperty, name_index);
        } else {
            self.emit_bytes(OpCode::GetProperty, name_index);
        }
    }

    fn index(&mut self) {
        let can_assign = self.can_assign;
        self.expression();
        self.consume(TokenType::RightBracket, "Expected ']' after index.");
        if can_assign && self.matches(TokenType::Equal) {
            self.expression();
            self.emit_byte(OpCode::SetIndex);
        } else {
            self.emit_byte(OpCode::GetIndex);
        }
    }

    fn variable(&mut self) {
        let name = self.previous.lexeme;

        let (get_op, set_op, arg) = if let Some(local_slot) = self.resolve_local(name) {
            (OpCode::GetLocal, OpCode::SetLocal, local_slot)
        } else if let Some(slot) = self.resolve_upvalue(name) {
            (OpCode::GetUpvalue, OpCode::SetUpvalue, slot)
        } else {
            (OpCode::GetGlobal, OpCode::SetGlobal, self.global_slot(name))
        };

        if self.can_assign && self.matches(TokenType::Equal) {
            self.expression();
            self.emit_bytes(set_op, arg);
        } else {
            self.emit_bytes(get_op, arg);
        }
    }

    fn function(&mut self) {
        self.begin_function();
        self.begin_scope();
        self.consume(TokenType::LeftParen, "Expected '(' after function name.");
        if !self.check(TokenType::RightParen) {
            for arity in 1.. {
                self.current_context_mut().function.arity = arity;
                if arity >= 255 {
                    self.error_at_current("Can't have more than 255 parameters.");
                }

                let constant = self.parse_variable("Expected parameter name.");
                self.define_variable(constant);

                if !self.matches(TokenType::Comma) {
                    break;
                }
            }
        }
        self.consume(TokenType::RightParen, "Expected ')' after parameters.");
        self.consume(TokenType::LeftBrace, "Expected '{' before function body.");
        self.block();
        let function_context = self.end_function();
        let func = function_context.function;
        let upvalues = function_context.upvalues;
        let func_ref = self.heap.alloc_raw(func);
        let idx = self.make_constant(Value::Function(func_ref));
        self.emit_bytes(OpCode::Closure, idx);

        for uv in &upvalues {
            self.emit_byte(uv.is_local as u8);
            self.emit_byte(uv.index);
        }
    }

    fn class_declaration(&mut self) {
        let global_idx = self.parse_variable("Expected class name.");
        let name = self.heap.intern(&self.previous.lexeme);
        let name_const = self.make_constant(Value::String(name));
        self.emit_bytes(OpCode::Class, name_const);
        self.define_variable(global_idx);
        self.consume(TokenType::LeftBrace, "Expected '{' before class body.");
        self.consume(TokenType::RightBrace, "Expected '}' after class body.");
    }

    fn string(&mut self) {
        let lex = self.previous.lexeme;
        let str = &lex[1..lex.len() - 1];
        let str_ref = self.heap.intern(str);
        self.emit_const(Value::String(str_ref));
    }

    fn parse_precedence(&mut self, precedence: Precedence) {
        self.advance();

        let prefix_rule = Compiler::get_rule(self.previous.kind).prefix;
        self.can_assign = precedence <= Precedence::Assignment;
        if let Some(prefix) = prefix_rule {
            prefix(self);
        } else {
            self.error_at_current("Expected expression.");
        }

        while precedence <= Compiler::get_rule(self.current.kind).precedence {
            self.advance();
            let infix_rule = Compiler::get_rule(self.previous.kind).infix;
            infix_rule.unwrap()(self);
        }

        if self.can_assign && self.matches(TokenType::Equal) {
            self.error_at_current("Invalid assignment target.");
        }
    }

    fn error_at_current(&mut self, message: &str) {
        if self.panic_mode {
            return;
        }
        self.panic_mode = true;
        self.had_error = true;
        let token = &self.previous;
        let location = match token.kind {
            TokenType::EOF => ErrorLocation::EndOfFile,
            TokenType::Error => ErrorLocation::Unknown,
            _ => ErrorLocation::Lexeme(token.lexeme.to_string()),
        };

        self.errors.push(CompileError {
            message: message.to_string(),
            line: token.line,
            location,
        });
    }

    fn get_rule(token_type: TokenType) -> ParseRule<'src> {
        match token_type {
            TokenType::LeftParen => ParseRule {
                prefix: Some(Compiler::grouping),
                infix: Some(Compiler::call),
                precedence: Precedence::Call,
            },
            TokenType::Minus => ParseRule {
                prefix: Some(Compiler::unary),
                infix: Some(Compiler::binary),
                precedence: Precedence::Term,
            },
            TokenType::Plus => ParseRule {
                prefix: None,
                infix: Some(Compiler::binary),
                precedence: Precedence::Term,
            },
            TokenType::Slash | TokenType::Star => ParseRule {
                prefix: None,
                infix: Some(Compiler::binary),
                precedence: Precedence::Factor,
            },
            TokenType::Number => ParseRule {
                prefix: Some(Compiler::number),
                infix: None,
                precedence: Precedence::None,
            },
            TokenType::False => ParseRule {
                prefix: Some(Compiler::literal),
                infix: None,
                precedence: Precedence::None,
            },
            TokenType::True => ParseRule {
                prefix: Some(Compiler::literal),
                infix: None,
                precedence: Precedence::None,
            },
            TokenType::Nil => ParseRule {
                prefix: Some(Compiler::literal),
                infix: None,
                precedence: Precedence::None,
            },
            TokenType::Bang => ParseRule {
                prefix: Some(Compiler::unary),
                infix: None,
                precedence: Precedence::None,
            },
            TokenType::BangEqual | TokenType::EqualEqual => ParseRule {
                prefix: None,
                infix: Some(Compiler::binary),
                precedence: Precedence::Equality,
            },
            TokenType::Greater
            | TokenType::GreaterEqual
            | TokenType::Less
            | TokenType::LessEqual => ParseRule {
                prefix: None,
                infix: Some(Compiler::binary),
                precedence: Precedence::Comparison,
            },
            TokenType::Identifier => ParseRule {
                prefix: Some(Compiler::variable),
                infix: None,
                precedence: Precedence::None,
            },
            TokenType::String => ParseRule {
                prefix: Some(Compiler::string),
                infix: None,
                precedence: Precedence::None,
            },
            TokenType::And => ParseRule {
                prefix: None,
                infix: Some(Compiler::and),
                precedence: Precedence::And,
            },
            TokenType::Or => ParseRule {
                prefix: None,
                infix: Some(Compiler::or),
                precedence: Precedence::Or,
            },
            TokenType::Dot => ParseRule {
                prefix: None,
                infix: Some(Compiler::dot),
                precedence: Precedence::Call,
            },
            TokenType::LeftBracket => ParseRule {
                prefix: None,
                infix: Some(Compiler::index),
                precedence: Precedence::Call,
            },
            _ => ParseRule {
                prefix: None,
                infix: None,
                precedence: Precedence::None,
            },
        }
    }

    fn current_chunk_mut(&mut self) -> &mut Chunk {
        &mut self.current_context_mut().function.chunk
    }
    fn current_context_mut(&mut self) -> &mut FunctionContext<'src> {
        self.contexts.last_mut().unwrap()
    }

    fn current_context(&self) -> &FunctionContext<'src> {
        self.contexts.last().unwrap()
    }

    fn mark_initialized(&mut self) {
        if self.current_context_mut().scope_depth != 0 {
            self.current_context_mut().locals.last_mut().unwrap().depth =
                self.current_context_mut().scope_depth;
        }
    }
}
