#[derive(Debug, PartialEq, Copy, Clone)]
pub enum TokenType {
    EOF,
    Error,

    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    Comma,
    Dot,
    Minus,
    Plus,
    Colon,
    Semicolon,
    Slash,
    Star,

    Bang,
    BangEqual,
    Equal,
    EqualEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,

    Identifier,
    String,
    Number,
    And,
    Class,
    Else,
    False,
    For,
    Fun,
    Switch,
    Case,
    Default,
    If,
    Nil,
    Or,
    Print,
    True,
    Var,
    While,
    Return,
    Super,
    This,

    Delete,

    Dummy,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Token<'src> {
    pub lexeme: &'src str,
    pub line: usize,
    pub kind: TokenType,
}

impl<'src> Default for Token<'src> {
    fn default() -> Self {
        Token {
            kind: TokenType::Dummy,
            lexeme: "",
            line: 0,
        }
    }
}

#[derive(Debug)]
pub struct Scanner<'src> {
    source: &'src str,
    bytes: &'src [u8],
    start: usize,
    current: usize,
    line: usize,
}

impl<'src> Scanner<'src> {
    pub fn new(source: &'src str) -> Self {
        Self {
            source,
            bytes: source.as_bytes(),
            start: 0,
            current: 0,
            line: 1,
        }
    }
    pub fn scan_token(&mut self) -> Token<'src> {
        self.skip_whitespace();
        self.start = self.current;

        if self.is_at_end() {
            return self.make_token(TokenType::EOF);
        }

        let c = self.advance();

        let token_type = match c {
            b'(' => TokenType::LeftParen,
            b')' => TokenType::RightParen,
            b'[' => TokenType::LeftBracket,
            b']' => TokenType::RightBracket,
            b'{' => TokenType::LeftBrace,
            b'}' => TokenType::RightBrace,
            b':' => TokenType::Colon,
            b';' => TokenType::Semicolon,
            b',' => TokenType::Comma,
            b'.' => TokenType::Dot,
            b'-' => TokenType::Minus,
            b'+' => TokenType::Plus,
            b'/' => TokenType::Slash,
            b'*' => TokenType::Star,
            b'!' => {
                if self.match_next(b'=') {
                    TokenType::BangEqual
                } else {
                    TokenType::Bang
                }
            }
            b'=' => {
                if self.match_next(b'=') {
                    TokenType::EqualEqual
                } else {
                    TokenType::Equal
                }
            }
            b'<' => {
                if self.match_next(b'=') {
                    TokenType::LessEqual
                } else {
                    TokenType::Less
                }
            }
            b'>' => {
                if self.match_next(b'=') {
                    TokenType::GreaterEqual
                } else {
                    TokenType::Greater
                }
            }
            b'0'..=b'9' => self.number(),
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => self.identifier(),
            b'"' => self.string(),
            _ => TokenType::Error,
        };

        self.make_token(token_type)
    }

    fn is_at_end(&self) -> bool {
        self.current >= self.bytes.len()
    }

    fn string(&mut self) -> TokenType {
        loop {
            let c = self.peek();
            if c == b'"' {
                self.advance();
                break;
            } else if c == b'\n' {
                self.line += 1;
            } else if self.is_at_end() {
                return TokenType::Error;
            }
            self.advance();
        }

        TokenType::String
    }

    fn number(&mut self) -> TokenType {
        while self.peek().is_ascii_digit() {
            self.advance();
        }

        // decimal part
        if self.peek() == b'.' && self.peek_next().is_ascii_digit() {
            self.advance();
            while self.peek().is_ascii_digit() {
                self.advance();
            }
        }

        TokenType::Number
    }

    fn identifier(&mut self) -> TokenType {
        while self.peek().is_ascii_alphanumeric() {
            self.advance();
        }

        match self.bytes[self.start] {
            b'a' => self.check_keyword(1, b"nd", TokenType::And),
            b'c' => match self.bytes.get(self.start + 1) {
                Some(b'a') => self.check_keyword(2, b"se", TokenType::Case),
                Some(b'l') => self.check_keyword(2, b"ass", TokenType::Class),
                _ => TokenType::Identifier,
            },
            b'd' => match self.bytes.get(self.start + 1) {
                Some(b'e') => match self.bytes.get(self.start + 2) {
                    Some(b'f') => self.check_keyword(3, b"ault", TokenType::Default),
                    Some(b'l') => self.check_keyword(3, b"ete", TokenType::Delete),
                    _ => TokenType::Identifier,
                },

                _ => TokenType::Identifier,
            },
            b'e' => self.check_keyword(1, b"lse", TokenType::Else),
            b'f' => match self.bytes.get(self.start + 1) {
                Some(b'a') => self.check_keyword(2, b"lse", TokenType::False),
                Some(b'o') => self.check_keyword(2, b"r", TokenType::For),
                Some(b'u') => self.check_keyword(2, b"n", TokenType::Fun),
                _ => TokenType::Identifier,
            },
            b'i' => self.check_keyword(1, b"f", TokenType::If),
            b'n' => self.check_keyword(1, b"il", TokenType::Nil),
            b'o' => self.check_keyword(1, b"r", TokenType::Or),
            b'p' => self.check_keyword(1, b"rint", TokenType::Print),
            b'r' => self.check_keyword(1, b"eturn", TokenType::Return),
            b's' => match self.bytes.get(self.start + 1) {
                Some(b'u') => self.check_keyword(2, b"per", TokenType::Super),
                Some(b'w') => self.check_keyword(2, b"itch", TokenType::Switch),
                _ => TokenType::Identifier,
            },
            b't' => match self.bytes.get(self.start + 1) {
                Some(b'h') => self.check_keyword(2, b"is", TokenType::This),
                Some(b'r') => self.check_keyword(2, b"ue", TokenType::True),
                _ => TokenType::Identifier,
            },
            b'v' => self.check_keyword(1, b"ar", TokenType::Var),
            b'w' => self.check_keyword(1, b"hile", TokenType::While),
            _ => TokenType::Identifier,
        }
    }

    fn check_keyword(&mut self, start: usize, rest: &[u8], kind: TokenType) -> TokenType {
        let start_of_token = self.start + start;
        if self.current - self.start == start + rest.len()
            && self.bytes[start_of_token..self.current] == rest[0..rest.len()]
        {
            kind
        } else {
            TokenType::Identifier
        }
    }

    pub fn make_token(&self, kind: TokenType) -> Token<'src> {
        Token {
            kind,
            lexeme: &self.source[self.start..self.current],
            line: self.line,
        }
    }

    fn advance(&mut self) -> u8 {
        let c = self.bytes[self.current];
        self.current += 1;
        c
    }

    fn peek(&self) -> u8 {
        *self.bytes[self.current..].iter().next().unwrap_or(&0)
    }

    fn peek_next(&self) -> u8 {
        if self.current + 1 >= self.bytes.len() {
            0
        } else {
            self.bytes[self.current + 1]
        }
    }

    fn match_next(&mut self, expected: u8) -> bool {
        if self.is_at_end() {
            return false;
        }
        if self.bytes[self.current] != expected {
            return false;
        }
        self.current += 1;
        true
    }

    fn skip_whitespace(&mut self) {
        loop {
            let c = self.peek();
            match c {
                b' ' | b'\r' | b'\t' => {
                    self.advance();
                }
                b'\n' => {
                    self.line += 1;
                    self.advance();
                }
                b'/' => {
                    if self.peek_next() == b'/' {
                        while self.peek() != b'\n' && !self.is_at_end() {
                            self.advance();
                        }
                    } else {
                        break;
                    }
                }
                _ => break,
            };
        }
    }
}
