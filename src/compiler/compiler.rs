use core::panic::PanicMessage;

use crate::{
    common::{ObjString, Value, opcodes},
    compiler::scanner::{Scanner, Token, TokenType},
    vm::{Chunk, Interner},
};

#[derive(PartialEq, PartialOrd)]
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
type ParseFn<'src> = InfixFn<'src>;

struct ParseRule<'src> {
    prefix: Option<PrefixFn<'src>>,
    infix: Option<InfixFn<'src>>,
    precedence: Precedence,
}

#[derive(Debug)]
pub struct Compiler<'src> {
    chunk: Chunk,
    scanner: Scanner<'src>,
    previous: Token<'src>,
    current: Token<'src>,
    interner: &'src mut Interner,
    can_assign: bool,
    had_error: bool,
    panic_mode: bool,
}

impl<'src> Compiler<'src> {
    pub fn compile(source: &'src str, interner: &mut Interner) -> Chunk {
        let mut compiler = Compiler {
            chunk: Chunk::new(),
            scanner: Scanner::new(source),
            previous: Token::default(),
            current: Token::default(),
            interner,
            can_assign: false,
            had_error: false,
            panic_mode: false,
        };

        compiler.advance();
        while !compiler.matches(TokenType::EOF) {
            compiler.declaration();
        }
        compiler.end_compiler();
        compiler.chunk
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

    fn check_token_type(&self, kind: TokenType) -> bool {
        self.current.kind == kind
    }

    fn matches(&mut self, kind: TokenType) -> bool {
        if !self.check_token_type(kind) {
            return false;
        }
        self.advance();
        true
    }

    fn end_compiler(&mut self) {
        #[cfg(debug_assertions)]
        {
            if !self.had_error {
                let _ = self.chunk.disassemble("code");
            }
        }
        self.emit_return()
    }

    fn emit_byte(&mut self, byte: u8) {
        self.chunk.write_byte(byte);
    }

    fn emit_bytes(&mut self, byte1: u8, byte2: u8) {
        self.emit_byte(byte1);
        self.emit_byte(byte2);
    }

    fn emit_const(&mut self, value: Value) {
        let index = self.chunk.add_constant(value);
        self.emit_byte(opcodes::OpConstant);
        self.emit_byte(index);
    }

    fn emit_return(&mut self) {
        self.emit_byte(opcodes::OpReturn);
    }

    fn expression(&mut self) {
        self.parse_precedence(Precedence::Assignment)
    }

    fn identifier_constant(&mut self, name: &str) -> u8 {
        if let Some(x) = self.interner.get(name) {
            for (index, constant) in self.chunk.constants.iter().enumerate() {
                if let Value::String(name) = constant {
                    let ptr = x.as_ref() as *const ObjString;
                    if *name == ptr {
                        return index as u8;
                    }
                }
            }
        }
        let ptr = self.intern(name);
        self.chunk.add_constant(Value::String(ptr))
    }

    fn parse_variable(&mut self, message: &str) -> u8 {
        self.consume(TokenType::Identifier, message);
        self.identifier_constant(self.previous.lexeme)
    }

    fn define_variable(&mut self, global: u8) {
        self.emit_bytes(opcodes::OpDefineGlobal, global);
    }

    fn var_declaration(&mut self) {
        let global = self.parse_variable("Expected variable name.");
        if self.matches(TokenType::Equal) {
            self.expression();
        } else {
            self.emit_byte(opcodes::OpNil);
        }

        self.consume(
            TokenType::Semicolon,
            "Expected ';' after variable declaration.",
        );

        self.define_variable(global);
    }

    fn expression_statement(&mut self) {
        self.expression();
        self.consume(TokenType::Semicolon, "Expected ';' after expression.");
        self.emit_byte(opcodes::OpPop);
    }

    fn print_statement(&mut self) {
        self.expression();
        self.consume(TokenType::Semicolon, "Expected ';' after value.");
        self.emit_byte(opcodes::OpPrint);
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
        if self.matches(TokenType::Var) {
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
            TokenType::False => self.emit_byte(opcodes::OpFalse),
            TokenType::True => self.emit_byte(opcodes::OpTrue),
            TokenType::Nil => self.emit_byte(opcodes::OpNil),
            _ => unreachable!(),
        }
    }

    fn unary(&mut self) {
        let kind = self.previous.kind;

        self.parse_precedence(Precedence::Assignment);

        match kind {
            TokenType::Minus => self.emit_byte(opcodes::OpNegate),
            TokenType::Bang => self.emit_byte(opcodes::OpNot),
            _ => unreachable!(),
        }
    }

    fn binary(&mut self) {
        let kind = self.previous.kind;
        let rule = Compiler::get_rule(kind);
        self.parse_precedence(rule.precedence);

        match kind {
            TokenType::Plus => self.emit_byte(opcodes::OpAdd),
            TokenType::Minus => self.emit_byte(opcodes::OpSubtract),
            TokenType::Star => self.emit_byte(opcodes::OpMultiply),
            TokenType::Slash => self.emit_byte(opcodes::OpDivide),
            TokenType::BangEqual => self.emit_bytes(opcodes::OpEqual, opcodes::OpNot),
            TokenType::EqualEqual => self.emit_byte(opcodes::OpEqual),
            TokenType::Greater => self.emit_byte(opcodes::OpGreater),
            TokenType::GreaterEqual => self.emit_bytes(opcodes::OpLess, opcodes::OpNot),
            TokenType::Less => self.emit_byte(opcodes::OpLess),
            TokenType::LessEqual => self.emit_bytes(opcodes::OpGreater, opcodes::OpNot),
            _ => unreachable!(),
        }
    }

    fn variable(&mut self) {
        let arg = self.identifier_constant(self.previous.lexeme);

        if self.can_assign && self.matches(TokenType::Equal) {
            self.expression();
            self.emit_bytes(opcodes::OpSetGlobal, arg);
        } else {
            self.emit_bytes(opcodes::OpGetGlobal, arg);
        }
    }

    fn string(&mut self) {
        let lex = self.previous.lexeme;
        let str = &lex[1..lex.len() - 1];
        // let obj = alloc_owned_string(str.to_string());
        let obj = self.interner.intern(str);
        self.emit_const(Value::String(obj));
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
        eprint!("[line {}] Error", token.line);
        if token.kind == TokenType::EOF {
            eprint!(" at end");
        } else if token.kind == TokenType::Error {
        } else {
            eprint!(" at '{}'", token.lexeme);
        }
        eprintln!(": {message}");
    }

    fn get_rule(token_type: TokenType) -> ParseRule<'src> {
        match token_type {
            TokenType::LeftParen => ParseRule {
                prefix: Some(Compiler::grouping),
                infix: None,
                precedence: Precedence::None,
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
            _ => ParseRule {
                prefix: None,
                infix: None,
                precedence: Precedence::None,
            },
        }
    }

    pub fn intern(&mut self, e: &str) -> *const ObjString {
        self.interner.intern(e)
    }
}
