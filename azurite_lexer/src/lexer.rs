use crate::token::{Span, Token, TokenKind};

pub struct Lexer {
    chars: Vec<char>,
    pos: usize,
    line: usize,
    col: usize,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        Self {
            chars: input.chars().collect(),
            pos: 0,
            line: 1,
            col: 1,
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token>, String> {
        let mut tokens = Vec::new();
        loop {
            self.skip_whitespace_and_comments();
            if self.is_eof() {
                let span = self.current_span();
                tokens.push(Token::new(TokenKind::EOF, span));
                return Ok(tokens);
            }
            let token = self.next_token()?;
            tokens.push(token);
        }
    }

    fn next_token(&mut self) -> Result<Token, String> {
        let c = self.peek().unwrap();
        match c {
            '0'..='9' => self.read_number(),
            '"' => self.read_string(),
            '\'' => self.read_char(),
            'a'..='z' | 'A'..='Z' | '_' => self.read_identifier_or_keyword(),
            '+' => self.single_char_token(TokenKind::Plus),
            '-' => {
                if self.peek_next() == Some('>') {
                    self.bump();
                    self.bump();
                    Ok(Token::new(TokenKind::Arrow, self.prev_span(2)))
                } else {
                    self.single_char_token(TokenKind::Minus)
                }
            }
            '*' => self.single_char_token(TokenKind::Star),
            '/' => self.single_char_token(TokenKind::Slash),
            '%' => self.single_char_token(TokenKind::Percent),
            '=' => {
                if self.peek_next() == Some('>') {
                    self.bump();
                    self.bump();
                    Ok(Token::new(TokenKind::FatArrow, self.prev_span(2)))
                } else {
                    self.try_two_char('=', TokenKind::Assign, TokenKind::Equal)
                }
            }
            '!' => self.try_two_char('=', TokenKind::Not, TokenKind::NotEqual),
            '<' => {
                if self.peek_next() == Some('<') {
                    self.bump();
                    self.bump();
                    Ok(Token::new(TokenKind::Shl, self.prev_span(2)))
                } else {
                    self.try_two_char('=', TokenKind::Less, TokenKind::LessEqual)
                }
            }
            '>' => {
                if self.peek_next() == Some('>') {
                    self.bump();
                    self.bump();
                    Ok(Token::new(TokenKind::Shr, self.prev_span(2)))
                } else {
                    self.try_two_char('=', TokenKind::Greater, TokenKind::GreaterEqual)
                }
            }
            '&' => self.try_two_char('&', TokenKind::BitAnd, TokenKind::AndAnd),
            '|' => self.try_two_char('|', TokenKind::BitOr, TokenKind::OrOr),
            '^' => self.single_char_token(TokenKind::BitXor),
            '(' => self.single_char_token(TokenKind::LParen),
            ')' => self.single_char_token(TokenKind::RParen),
            '{' => self.single_char_token(TokenKind::LBrace),
            '}' => self.single_char_token(TokenKind::RBrace),
            '[' => self.single_char_token(TokenKind::LBracket),
            ']' => self.single_char_token(TokenKind::RBracket),
            ',' => self.single_char_token(TokenKind::Comma),
            ';' => self.single_char_token(TokenKind::Semicolon),
            ':' => self.single_char_token(TokenKind::Colon),
            '.' => {
                if self.peek_next() == Some('.') {
                    self.bump();
                    self.bump();
                    Ok(Token::new(TokenKind::DotDot, self.prev_span(2)))
                } else {
                    self.single_char_token(TokenKind::Dot)
                }
            }
            '#' => self.single_char_token(TokenKind::Hash),
            '?' => {
                let start = self.pos;
                let line = self.line;
                let col = self.col;
                self.bump();
                if self.peek() == Some('.') {
                    self.bump();
                    return Ok(Token::new(TokenKind::QuestionDot, Span::new(start, self.pos, line, col)));
                }
                return Ok(Token::new(TokenKind::Question, Span::new(start, self.pos, line, col)));
            }
            _ => {
                let span = self.current_span();
                let err = format!("unexpected character '{}'", c);
                self.bump();
                Ok(Token::new(TokenKind::Error(err), span))
            }
        }
    }

    // --- Number literal ---
    fn read_number(&mut self) -> Result<Token, String> {
        let start = self.pos;
        let start_line = self.line;
        let start_col = self.col;
        let mut is_float = false;

        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                self.bump();
            } else if c == '.' && !is_float && self.peek_next().map_or(false, |n| n.is_ascii_digit()) {
                is_float = true;
                self.bump();
            } else {
                break;
            }
        }

        let slice: String = self.chars[start..self.pos].iter().collect();
        let span = Span::new(start, self.pos, start_line, start_col);

        if is_float {
            let val: f64 = slice.parse().map_err(|_| format!("invalid float literal: {}", slice))?;
            Ok(Token::new(TokenKind::Float(val), span))
        } else {
            let val: i64 = slice.parse().map_err(|_| format!("invalid int literal: {}", slice))?;
            Ok(Token::new(TokenKind::Int(val), span))
        }
    }

    // --- String literal ---
    fn read_string(&mut self) -> Result<Token, String> {
        let start = self.pos;
        let start_line = self.line;
        let start_col = self.col;
        self.bump(); // skip opening "

        let mut value = String::new();
        loop {
            match self.peek() {
                None => {
                    return Err(format!(
                        "unterminated string literal at line {}, col {}",
                        start_line, start_col
                    ));
                }
                Some('"') => {
                    self.bump();
                    let span = Span::new(start, self.pos, start_line, start_col);
                    return Ok(Token::new(TokenKind::String(value.into()), span));
                }
                Some('\\') => {
                    self.bump();
                    let escaped = self.parse_escape()?;
                    value.push(escaped);
                }
                Some(c) => {
                    value.push(c);
                    self.bump();
                }
            }
        }
    }

    // --- Char literal ---
    fn read_char(&mut self) -> Result<Token, String> {
        let start = self.pos;
        let start_line = self.line;
        let start_col = self.col;
        self.bump(); // skip opening '

        let c = match self.peek() {
            None => return Err("unterminated char literal".to_string()),
            Some('\\') => {
                self.bump();
                self.parse_escape()?
            }
            Some(c) => {
                self.bump();
                c
            }
        };

        match self.peek() {
            Some('\'') => {
                self.bump();
                let span = Span::new(start, self.pos, start_line, start_col);
                Ok(Token::new(TokenKind::Char(c), span))
            }
            _ => Err(format!(
                "unterminated char literal at line {}, col {}",
                start_line, start_col
            )),
        }
    }

    // --- Identifier or keyword ---
    fn read_identifier_or_keyword(&mut self) -> Result<Token, String> {
        let start = self.pos;
        let start_line = self.line;
        let start_col = self.col;

        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' {
                self.bump();
            } else {
                break;
            }
        }

        let ident: String = self.chars[start..self.pos].iter().collect();
        let span = Span::new(start, self.pos, start_line, start_col);
        let kind = match ident.as_str() {
            "let" => TokenKind::Let,
            "func" => TokenKind::Func,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "while" => TokenKind::While,
            "for" => TokenKind::For,
            "match" => TokenKind::Match,
            "return" => TokenKind::Return,
            "break" => TokenKind::Break,
            "continue" => TokenKind::Continue,
            "import" => TokenKind::Import,
            "struct" => TokenKind::Struct,
            "enum" => TokenKind::Enum,
            "class" => TokenKind::Class,
            "self" => TokenKind::Self_,
            "super" => TokenKind::Super,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "null" => TokenKind::Null,
            "and" => TokenKind::And,
            "or" => TokenKind::Or,
            "not" => TokenKind::Not,
            _ => TokenKind::Ident(ident.into()),
        };
        Ok(Token::new(kind, span))
    }

    // --- Helpers ---

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_next(&self) -> Option<char> {
        self.chars.get(self.pos + 1).copied()
    }

    fn bump(&mut self) {
        if let Some(c) = self.chars.get(self.pos) {
            self.pos += 1;
            if *c == '\n' {
                self.line += 1;
                self.col = 1;
            } else {
                self.col += 1;
            }
        }
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.chars.len()
    }

    fn parse_escape(&mut self) -> Result<char, String> {
        match self.peek() {
            Some('n') => { self.bump(); Ok('\n') }
            Some('t') => { self.bump(); Ok('\t') }
            Some('r') => { self.bump(); Ok('\r') }
            Some('\\') => { self.bump(); Ok('\\') }
            Some('"') => { self.bump(); Ok('"') }
            Some('\'') => { self.bump(); Ok('\'') }
            Some('0') => { self.bump(); Ok('\0') }
            Some(c) => Err(format!("invalid escape char '\\{}'", c)),
            None => Err("unterminated escape sequence".to_string()),
        }
    }

    fn current_span(&self) -> Span {
        Span::new(self.pos, self.pos, self.line, self.col)
    }

    fn prev_span(&self, len: usize) -> Span {
        Span::new(self.pos - len, self.pos, self.line, self.col)
    }

    fn single_char_token(&mut self, kind: TokenKind) -> Result<Token, String> {
        let start = self.pos;
        let line = self.line;
        let col = self.col;
        self.bump();
        Ok(Token::new(kind, Span::new(start, self.pos, line, col)))
    }

    fn try_two_char(
        &mut self,
        expected: char,
        single: TokenKind,
        double: TokenKind,
    ) -> Result<Token, String> {
        let start = self.pos;
        let line = self.line;
        let col = self.col;
        if self.peek_next() == Some(expected) {
            self.bump();
            self.bump();
            Ok(Token::new(double, Span::new(start, self.pos, line, col)))
        } else {
            self.bump();
            Ok(Token::new(single, Span::new(start, self.pos, line, col)))
        }
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            match self.peek() {
                Some(c) if c.is_whitespace() => {
                    self.bump();
                }
                Some('/') if self.peek_next() == Some('/') => {
                    while let Some(c) = self.peek() {
                        if c == '\n' {
                            break;
                        }
                        self.bump();
                    }
                }
                Some('/') if self.peek_next() == Some('*') => {
                    self.bump();
                    self.bump();
                    loop {
                        match self.peek() {
                            None => return,
                            Some('*') if self.peek_next() == Some('/') => {
                                self.bump();
                                self.bump();
                                break;
                            }
                            _ => {
                                self.bump();
                            }
                        }
                    }
                }
                _ => break,
            }
        }
    }
}
