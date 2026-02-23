use core::panic;

use crate::{
    common::{Value, alloc_owned_string, opcodes},
    compiler::scanner::{Scanner, Token, TokenType},
    vm::{Chunk, vm::VMResult},
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

type PrefixFn<'src, 'a> = fn(&mut Compiler<'src, 'a>);
type InfixFn<'src, 'a> = PrefixFn<'src, 'a>;
type ParseFn<'src, 'a> = InfixFn<'src, 'a>;

struct ParseRule<'src, 'a> {
    prefix: Option<PrefixFn<'src, 'a>>,
    infix: Option<InfixFn<'src, 'a>>,
    precedence: Precedence,
}

#[derive(Debug)]
pub struct Compiler<'src, 'a> {
    source: &'src str,
    chunk: &'a mut Chunk,
    scanner: Scanner<'src>,
    previous: Token<'src>,
    current: Token<'src>,
    had_error: bool,
}

impl<'src, 'a> Compiler<'src, 'a> {
    pub fn compile(source: &'src str, chunk: &mut Chunk) -> VMResult {
        let mut compiler = Compiler {
            source,
            chunk,
            scanner: Scanner::new(source),
            previous: Token::default(),
            current: Token::default(),
            had_error: false,
        };

        compiler.advance();
        compiler.expression();
        compiler.consume(TokenType::EOF, "Expected end of expression");
        compiler.end_compiler();
        Ok(())
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

    fn string(&mut self) {
        let lex = self.previous.lexeme;
        let str = &lex[1..lex.len() - 1];
        let obj = alloc_owned_string(str.to_string());
        self.emit_const(Value::String(obj));
    }

    fn parse_precedence(&mut self, precedence: Precedence) {
        self.advance();

        let prefix_rule = Compiler::get_rule(self.previous.kind).prefix;

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
    }

    fn error_at_current(&self, message: &str) {
        let token = &self.previous;
        eprint!("[line {}] Error", token.line);
        if token.kind == TokenType::EOF {
            eprint!(" at end");
        } else if token.kind == TokenType::Error {
        } else {
            eprint!(" at '{}'", token.lexeme);
        }
        eprintln!(": {message}");

        panic!("Compiler State: {:?}", self);
    }

    fn get_rule(token_type: TokenType) -> ParseRule<'src, 'a> {
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
}
