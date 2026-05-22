use azurite_lexer::{Lexer, Span, Token, TokenKind};
use crate::ast::*;

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    pub fn from_source(source: &str) -> Result<(Self, Vec<Token>), String> {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize()?;
        Ok((Self::new(tokens.clone()), tokens))
    }

    pub fn parse_program(&mut self) -> Result<Program, String> {
        let mut statements = Vec::new();
        while !self.is_eof() {
            let stmt = self.parse_stmt()?;
            statements.push(stmt);
        }
        Ok(Program { statements })
    }

    fn parse_stmt(&mut self) -> Result<Stmt, String> {
        match self.peek_kind() {
            Some(TokenKind::Let) => self.parse_let(),
            Some(TokenKind::Func) => self.parse_func(),
            Some(TokenKind::If) => {
                let expr = self.parse_if_expr()?;
                if let Expr::If { condition, then_branch, else_branch } = expr {
                    Ok(Stmt::If { condition, then_branch, else_branch })
                } else {
                    unreachable!()
                }
            }
            Some(TokenKind::While) => {
                let expr = self.parse_while_expr()?;
                if let Expr::While { condition, body } = expr {
                    Ok(Stmt::While { condition, body })
                } else {
                    unreachable!()
                }
            }
            Some(TokenKind::Return) => self.parse_return(),
            _ => {
                let expr = self.parse_expr(0)?;
                self.expect_semicolon()?;
                Ok(Stmt::Expr(expr))
            }
        }
    }

    fn parse_let(&mut self) -> Result<Stmt, String> {
        self.advance(); // consume 'let'
        let name = self.parse_ident()?;
        let type_annotation = if self.peek_kind() == Some(TokenKind::Colon) {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };
        self.expect(TokenKind::Assign, "expected '=' in let")?;
        let value = self.parse_expr(0)?;
        self.expect_semicolon()?;
        Ok(Stmt::Let { name, type_annotation, value: Box::new(value) })
    }

    fn parse_func(&mut self) -> Result<Stmt, String> {
        self.advance(); // consume 'func'
        let name = self.parse_ident()?;
        self.expect(TokenKind::LParen, "expected '(' after func name")?;
        let params = self.parse_params()?;
        self.expect(TokenKind::RParen, "expected ')' after params")?;
        let return_type = if self.peek_kind() == Some(TokenKind::Arrow) {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };
        let body = self.parse_block()?;
        Ok(Stmt::Func { name, params, return_type, body: Box::new(body) })
    }

    fn parse_params(&mut self) -> Result<Vec<Param>, String> {
        let mut params = Vec::new();
        loop {
            match self.peek_kind() {
                Some(TokenKind::RParen) | None => break,
                Some(TokenKind::Comma) => { self.advance(); }
                _ => {
                    let name = self.parse_ident()?;
                    let type_annotation = if self.peek_kind() == Some(TokenKind::Colon) {
                        self.advance();
                        Some(self.parse_type()?)
                    } else {
                        None
                    };
                    params.push(Param { name, type_annotation });
                }
            }
        }
        Ok(params)
    }

    fn parse_return(&mut self) -> Result<Stmt, String> {
        self.advance(); // consume 'return'
        let value = match self.peek_kind() {
            Some(TokenKind::Semicolon) | Some(TokenKind::EOF) | Some(TokenKind::RBrace) => None,
            _ => Some(Box::new(self.parse_expr(0)?)),
        };
        self.expect_semicolon()?;
        Ok(Stmt::Return { value })
    }

    fn parse_type(&mut self) -> Result<Type, String> {
        match self.peek_kind() {
            Some(TokenKind::Ident(name)) => {
                let name = name.clone();
                self.advance();
                Ok(Type::Name(name))
            }
            _ => Err(format!("expected type, found {:?}", self.peek_kind())),
        }
    }

    // --- Expression parsing (Pratt parser) ---

    fn parse_expr(&mut self, min_bp: u8) -> Result<Expr, String> {
        let mut lhs = match self.peek_kind() {
            Some(TokenKind::Int(n)) => { self.advance(); Expr::Int(n) }
            Some(TokenKind::Float(n)) => { self.advance(); Expr::Float(n) }
            Some(TokenKind::String(s)) => { self.advance(); Expr::String(s) }
            Some(TokenKind::Char(c)) => { self.advance(); Expr::Char(c) }
            Some(TokenKind::True) => { self.advance(); Expr::Bool(true) }
            Some(TokenKind::False) => { self.advance(); Expr::Bool(false) }
            Some(TokenKind::Null) => { self.advance(); Expr::Null }
            Some(TokenKind::Ident(name)) => {
                let ident = Ident { name: name.clone(), span: self.current_span() };
                self.advance();
                Expr::Ident(ident)
            }
            Some(TokenKind::LParen) => {
                self.advance();
                let expr = self.parse_expr(0)?;
                self.expect(TokenKind::RParen, "expected ')'")?;
                expr
            }
            Some(TokenKind::LBrace) => self.parse_block()?,
            Some(TokenKind::If) => self.parse_if_expr()?,
            Some(TokenKind::While) => self.parse_while_expr()?,
            Some(TokenKind::Minus) | Some(TokenKind::Not) => {
                let op = match self.peek_kind() {
                    Some(TokenKind::Minus) => UnOp::Neg,
                    _ => UnOp::Not,
                };
                self.advance();
                let ((), r_bp) = prefix_binding_power(op);
                let operand = self.parse_expr(r_bp)?;
                Expr::Unary { op, operand: Box::new(operand) }
            }
            Some(other) => return Err(format!("unexpected token in expression: {}", other)),
            None => return Err("unexpected end of input in expression".to_string()),
        };

        loop {
            match self.peek_kind() {
                Some(TokenKind::LParen) => {
                    self.advance();
                    let mut args = Vec::new();
                    loop {
                        match self.peek_kind() {
                            Some(TokenKind::RParen) | None => break,
                            Some(TokenKind::Comma) => { self.advance(); }
                            _ => {
                                let arg = self.parse_expr(0)?;
                                args.push(arg);
                            }
                        }
                    }
                    self.expect(TokenKind::RParen, "expected ')' after call args")?;
                    lhs = Expr::Call { callee: Box::new(lhs), args };
                }
                Some(op_kind) if is_binop(&op_kind) => {
                    let op = token_to_binop(op_kind).unwrap();
                    let (l_bp, r_bp) = infix_binding_power(op);
                    if l_bp < min_bp {
                        break;
                    }
                    self.advance();
                    let rhs = self.parse_expr(r_bp)?;
                    lhs = Expr::Binary { left: Box::new(lhs), op, right: Box::new(rhs) };
                }
                _ => break,
            }
        }

        Ok(lhs)
    }

    fn parse_block(&mut self) -> Result<Expr, String> {
        self.expect(TokenKind::LBrace, "expected '{'")?;
        let mut statements = Vec::new();
        loop {
            match self.peek_kind() {
                Some(TokenKind::RBrace) | None => break,
                _ => {
                    let stmt = self.parse_stmt()?;
                    statements.push(stmt);
                }
            }
        }
        self.expect(TokenKind::RBrace, "expected '}'")?;
        Ok(Expr::Block(statements))
    }

    fn parse_if_expr(&mut self) -> Result<Expr, String> {
        self.advance(); // consume 'if'
        let condition = self.parse_expr(0)?;
        let then_branch = self.parse_block()?;
        let else_branch = if self.peek_kind() == Some(TokenKind::Else) {
            self.advance();
            if self.peek_kind() == Some(TokenKind::If) {
                Some(Box::new(self.parse_if_expr()?))
            } else {
                Some(Box::new(self.parse_block()?))
            }
        } else {
            None
        };
        Ok(Expr::If { condition: Box::new(condition), then_branch: Box::new(then_branch), else_branch })
    }

    fn parse_while_expr(&mut self) -> Result<Expr, String> {
        self.advance(); // consume 'while'
        let condition = self.parse_expr(0)?;
        let body = self.parse_block()?;
        Ok(Expr::While { condition: Box::new(condition), body: Box::new(body) })
    }

    // --- Helpers ---

    fn peek_kind(&self) -> Option<TokenKind> {
        self.tokens.get(self.pos).map(|t| t.kind.clone())
    }

    fn current_span(&self) -> Span {
        self.tokens.get(self.pos).map_or(
            Span::new(0, 0, 0, 0),
            |t| t.span,
        )
    }

    fn advance(&mut self) {
        self.pos += 1;
    }

    fn is_eof(&self) -> bool {
        matches!(self.peek_kind(), Some(TokenKind::EOF) | None)
    }

    fn expect(&mut self, expected: TokenKind, msg: &str) -> Result<(), String> {
        match self.peek_kind() {
            Some(ref kind) if *kind == expected => {
                self.advance();
                Ok(())
            }
            Some(other) => Err(format!("{}: expected {}, found {}", msg, expected, other)),
            None => Err(format!("{}: expected {}, found EOF", msg, expected)),
        }
    }

    fn expect_semicolon(&mut self) -> Result<(), String> {
        // Semicolons are optional — accept them silently when present
        if self.peek_kind() == Some(TokenKind::Semicolon) {
            self.advance();
        }
        Ok(())
    }

    fn parse_ident(&mut self) -> Result<Ident, String> {
        match self.peek_kind() {
            Some(TokenKind::Ident(name)) => {
                let span = self.current_span();
                let ident = Ident { name: name.clone(), span };
                self.advance();
                Ok(ident)
            }
            Some(other) => Err(format!("expected identifier, found {}", other)),
            None => Err("expected identifier, found EOF".to_string()),
        }
    }
}

// --- Binding power ---

fn prefix_binding_power(op: UnOp) -> ((), u8) {
    match op {
        UnOp::Neg => ((), 9),
        UnOp::Not => ((), 9),
    }
}

fn infix_binding_power(op: BinOp) -> (u8, u8) {
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

fn is_binop(kind: &TokenKind) -> bool {
    matches!(kind,
        TokenKind::Plus | TokenKind::Minus | TokenKind::Star | TokenKind::Slash
        | TokenKind::Percent | TokenKind::Assign | TokenKind::Equal | TokenKind::NotEqual
        | TokenKind::Less | TokenKind::Greater | TokenKind::LessEqual | TokenKind::GreaterEqual
        | TokenKind::AndAnd | TokenKind::OrOr | TokenKind::And | TokenKind::Or
        | TokenKind::BitAnd | TokenKind::BitOr | TokenKind::BitXor | TokenKind::Shl | TokenKind::Shr
    )
}

fn token_to_binop(kind: TokenKind) -> Option<BinOp> {
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


