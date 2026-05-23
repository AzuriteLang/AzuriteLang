use azurite_errors::AzError;
use azurite_lexer::TokenKind;
use crate::ast::{BinOp, Ident, Program, UnOp};

mod stmt;
pub mod expr;
mod pattern;
pub mod type_;

pub struct Parser {
    tokens: Vec<azurite_lexer::Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<azurite_lexer::Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    pub fn from_source(source: &str) -> Result<(Self, Vec<azurite_lexer::Token>), AzError> {
        let mut lexer = azurite_lexer::Lexer::new(source);
        let tokens = lexer.tokenize().map_err(|msg| {
            AzError::new(azurite_errors::ErrorKind::Lex, azurite_lexer::Span::new(0, 0, 1, 1), msg)
        })?;
        Ok((Self::new(tokens.clone()), tokens))
    }

    pub fn parse_program(&mut self) -> Result<Program, AzError> {
        let mut statements = Vec::new();
        while !self.is_eof() {
            statements.push(stmt::parse_stmt(self)?);
        }
        Ok(Program { statements })
    }

    pub fn err(&self, msg: impl Into<String>) -> AzError {
        AzError::new(azurite_errors::ErrorKind::Parse, self.current_span(), msg)
    }

    pub fn peek_kind(&self) -> Option<TokenKind> {
        self.tokens.get(self.pos).map(|t| t.kind.clone())
    }

    pub fn current_span(&self) -> azurite_lexer::Span {
        self.tokens.get(self.pos).map_or(
            azurite_lexer::Span::new(0, 0, 0, 0),
            |t| t.span,
        )
    }

    pub fn advance(&mut self) { self.pos += 1; }

    pub fn is_eof(&self) -> bool {
        matches!(self.peek_kind(), Some(TokenKind::EOF) | None)
    }

    pub fn expect(&mut self, expected: TokenKind, msg: &str) -> Result<(), AzError> {
        match self.peek_kind() {
            Some(k) if k == expected => { self.advance(); Ok(()) }
            Some(other) => Err(self.err(format!("{}: expected {}, found {}", msg, expected, other))),
            None => Err(self.err(format!("{}: expected {}, found EOF", msg, expected))),
        }
    }

    pub fn expect_semicolon(&mut self) -> Result<(), AzError> {
        if self.peek_kind() == Some(TokenKind::Semicolon) { self.advance(); }
        Ok(())
    }

    pub fn parse_ident(&mut self) -> Result<Ident, AzError> {
        match self.peek_kind() {
            Some(TokenKind::Ident(name)) => {
                let span = self.current_span();
                let ident = Ident { name: name.to_string(), span };
                self.advance();
                Ok(ident)
            }
            Some(other) => Err(self.err(format!("expected identifier, found {}", other))),
            None => Err(self.err("expected identifier, found EOF")),
        }
    }

    pub fn parse_ident_or_self(&mut self) -> Result<Ident, AzError> {
        match self.peek_kind() {
            Some(TokenKind::Self_) => {
                let span = self.current_span();
                self.advance();
                Ok(Ident { name: "self".to_string(), span })
            }
            _ => self.parse_ident(),
        }
    }
}

// Binding power functions
pub fn prefix_binding_power(op: UnOp) -> ((), u8) {
    match op { UnOp::Neg => ((), 9), UnOp::Not => ((), 9) }
}

pub fn infix_binding_power(op: BinOp) -> (u8, u8) {
    match op {
        BinOp::Assign => (1, 2),
        BinOp::Or => (3, 4),
        BinOp::And => (5, 6),
        BinOp::Eq | BinOp::Neq => (7, 8),
        BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => (9, 10),
        BinOp::BitOr => (11, 12),
        BinOp::BitXor => (13, 14),
        BinOp::BitAnd => (15, 16),
        BinOp::Shl | BinOp::Shr => (17, 18),
        BinOp::Add | BinOp::Sub => (19, 20),
        BinOp::Mul | BinOp::Div | BinOp::Mod => (21, 22),
    }
}

pub fn is_binop(kind: &TokenKind) -> bool {
    matches!(kind,
        TokenKind::Plus | TokenKind::Minus | TokenKind::Star | TokenKind::Slash
        | TokenKind::Percent | TokenKind::Assign | TokenKind::Equal | TokenKind::NotEqual
        | TokenKind::Less | TokenKind::Greater | TokenKind::LessEqual | TokenKind::GreaterEqual
        | TokenKind::AndAnd | TokenKind::OrOr | TokenKind::And | TokenKind::Or
        | TokenKind::BitAnd | TokenKind::BitOr | TokenKind::BitXor | TokenKind::Shl | TokenKind::Shr
    )
}

pub fn is_comparison(op: BinOp) -> bool {
    matches!(op, BinOp::Eq | BinOp::Neq | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge)
}

pub fn token_to_binop(kind: TokenKind) -> Option<BinOp> {
    match kind {
        TokenKind::Plus => Some(BinOp::Add),
        TokenKind::Minus => Some(BinOp::Sub),
        TokenKind::Star => Some(BinOp::Mul),
        TokenKind::Slash => Some(BinOp::Div),
        TokenKind::Percent => Some(BinOp::Mod),
        TokenKind::Assign => Some(BinOp::Assign),
        TokenKind::Equal => Some(BinOp::Eq),
        TokenKind::NotEqual => Some(BinOp::Neq),
        TokenKind::Less => Some(BinOp::Lt),
        TokenKind::Greater => Some(BinOp::Gt),
        TokenKind::LessEqual => Some(BinOp::Le),
        TokenKind::GreaterEqual => Some(BinOp::Ge),
        TokenKind::AndAnd | TokenKind::And => Some(BinOp::And),
        TokenKind::OrOr | TokenKind::Or => Some(BinOp::Or),
        TokenKind::BitAnd => Some(BinOp::BitAnd),
        TokenKind::BitOr => Some(BinOp::BitOr),
        TokenKind::BitXor => Some(BinOp::BitXor),
        TokenKind::Shl => Some(BinOp::Shl),
        TokenKind::Shr => Some(BinOp::Shr),
        _ => None,
    }
}
