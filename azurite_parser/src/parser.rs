use azurite_errors::{AzError, ErrorKind};
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

    pub fn from_source(source: &str) -> Result<(Self, Vec<Token>), AzError> {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.tokenize().map_err(|msg| {
            AzError::new(ErrorKind::Lex, Span::new(0, 0, 1, 1), msg)
        })?;
        Ok((Self::new(tokens.clone()), tokens))
    }

    pub fn parse_program(&mut self) -> Result<Program, AzError> {
        let mut statements = Vec::new();
        while !self.is_eof() {
            let stmt = self.parse_stmt()?;
            statements.push(stmt);
        }
        Ok(Program { statements })
    }

    fn err(&self, msg: impl Into<String>) -> AzError {
        let span = self.current_span();
        AzError::new(ErrorKind::Parse, span, msg)
    }

    fn parse_stmt(&mut self) -> Result<Stmt, AzError> {
        match self.peek_kind() {
            Some(TokenKind::Let) => self.parse_let(),
            Some(TokenKind::Func) => self.parse_func(),
            Some(TokenKind::Class) => self.parse_class(),
            Some(TokenKind::Enum) => self.parse_enum(),
            Some(TokenKind::If) => {
                let expr = self.parse_if_expr()?;
                if let Expr::If { condition, then_branch, else_branch } = expr {
                    Ok(Stmt::If { condition, then_branch, else_branch })
                } else { unreachable!() }
            }
            Some(TokenKind::While) => {
                let expr = self.parse_while_expr()?;
                if let Expr::While { condition, body } = expr {
                    Ok(Stmt::While { condition, body })
                } else { unreachable!() }
            }
            Some(TokenKind::Import) => self.parse_import(),
            Some(TokenKind::For) => self.parse_for(),
            Some(TokenKind::Return) => self.parse_return(),
            _ => {
                let expr = self.parse_expr(0)?;
                self.expect_semicolon()?;
                Ok(Stmt::Expr(expr))
            }
        }
    }

    fn parse_enum(&mut self) -> Result<Stmt, AzError> {
        self.advance();
        let name = self.parse_ident()?;
        self.expect(TokenKind::LBrace, "expected '{' after enum name")?;
        let mut variants = Vec::new();
        loop {
            match self.peek_kind() {
                Some(TokenKind::RBrace) | None => break,
                Some(TokenKind::Comma) => { self.advance(); }
                _ => {
                    let vname = self.parse_ident()?;
                    let types = if self.peek_kind() == Some(TokenKind::LParen) {
                        self.advance();
                        let mut ts = Vec::new();
                        loop {
                            match self.peek_kind() {
                                Some(TokenKind::RParen) | None => break,
                                Some(TokenKind::Comma) => { self.advance(); }
                                _ => { ts.push(self.parse_type()?); }
                            }
                        }
                        self.expect(TokenKind::RParen, "expected ')'")?;
                        ts
                    } else { Vec::new() };
                    variants.push(EnumVariant { name: vname, types });
                }
            }
        }
        self.expect(TokenKind::RBrace, "expected '}'")?;
        Ok(Stmt::Enum { name, variants })
    }

    fn parse_class(&mut self) -> Result<Stmt, AzError> {
        self.advance();
        let name = self.parse_ident()?;
        self.expect(TokenKind::LBrace, "expected '{' after class name")?;

        let mut fields = Vec::new();
        let mut methods = Vec::new();

        loop {
            match self.peek_kind() {
                Some(TokenKind::RBrace) | None => break,
                _ => self.parse_class_member(&mut fields, &mut methods)?,
            }
        }
        self.expect(TokenKind::RBrace, "expected '}' after class body")?;
        Ok(Stmt::Class { name, fields, methods })
    }

    fn parse_class_member(&mut self, fields: &mut Vec<ClassField>, methods: &mut Vec<Stmt>) -> Result<(), AzError> {
        match self.peek_kind() {
            Some(TokenKind::Func) => {
                methods.push(self.parse_func()?);
            }
            Some(TokenKind::Ident(_)) => {
                // Parse as field: name: type
                let name = self.parse_ident()?;
                if self.peek_kind() == Some(TokenKind::Colon) {
                    self.advance();
                    let type_ = self.parse_type()?;
                    self.expect_semicolon()?;
                    fields.push(ClassField { name, type_ });
                } else {
                    return Err(self.err(format!("expected ':' after field name '{}'", name.name)));
                }
            }
            _ => {
                return Err(self.err(format!("unexpected token in class: {}", self.peek_kind().unwrap())));
            }
        }
        Ok(())
    }

    fn parse_let(&mut self) -> Result<Stmt, AzError> {
        self.advance();
        let name = self.parse_ident()?;
        let type_annotation = if self.peek_kind() == Some(TokenKind::Colon) {
            self.advance();
            Some(self.parse_type()?)
        } else { None };
        self.expect(TokenKind::Assign, "expected '=' in let declaration")?;
        let value = self.parse_expr(0)?;
        self.expect_semicolon()?;
        Ok(Stmt::Let { name, type_annotation, value: Box::new(value) })
    }

    fn parse_func(&mut self) -> Result<Stmt, AzError> {
        self.advance();
        let name = self.parse_ident()?;
        self.expect(TokenKind::LParen, "expected '(' after function name")?;
        let params = self.parse_params()?;
        self.expect(TokenKind::RParen, "expected ')' after parameters")?;
        let return_type = if self.peek_kind() == Some(TokenKind::Arrow) {
            self.advance();
            Some(self.parse_type()?)
        } else { None };
        let body = self.parse_block()?;
        Ok(Stmt::Func { name, params, return_type, body: Box::new(body) })
    }

    fn parse_params(&mut self) -> Result<Vec<Param>, AzError> {
        let mut params = Vec::new();
        loop {
            match self.peek_kind() {
                Some(TokenKind::RParen) | None => break,
                Some(TokenKind::Comma) => { self.advance(); }
                _ => {
                    let name = self.parse_ident_or_self()?;
                    let type_annotation = if self.peek_kind() == Some(TokenKind::Colon) {
                        self.advance();
                        Some(self.parse_type()?)
                    } else { None };
                    params.push(Param { name, type_annotation });
                }
            }
        }
        Ok(params)
    }

    fn parse_ident_or_self(&mut self) -> Result<Ident, AzError> {
        match self.peek_kind() {
            Some(TokenKind::Self_) => {
                let span = self.current_span();
                self.advance();
                Ok(Ident { name: "self".to_string(), span })
            }
            _ => self.parse_ident(),
        }
    }

    fn parse_import(&mut self) -> Result<Stmt, AzError> {
        let span = self.current_span();
        self.advance();
        match self.peek_kind() {
            Some(TokenKind::String(path)) => {
                let path = path.clone();
                self.advance();
                self.expect_semicolon()?;
                Ok(Stmt::Import { path, span })
            }
            _ => Err(self.err("expected string literal after 'import'")),
        }
    }

    fn parse_for(&mut self) -> Result<Stmt, AzError> {
        self.advance();
        let name = self.parse_ident()?;
        // Expect 'in' keyword (tokenized as Ident("in"))
        match self.peek_kind() {
            Some(TokenKind::Ident(ref s)) if s == "in" => { self.advance(); }
            Some(ref other) => return Err(self.err(format!("expected 'in' after for variable, found {}", other))),
            None => return Err(self.err("expected 'in' after for variable, found EOF")),
        }
        let iterable = self.parse_expr(0)?;
        let body = self.parse_block()?;
        Ok(Stmt::For { name, iterable: Box::new(iterable), body: Box::new(body) })
    }

    fn parse_return(&mut self) -> Result<Stmt, AzError> {
        self.advance();
        let value = match self.peek_kind() {
            Some(TokenKind::Semicolon) | Some(TokenKind::EOF) | Some(TokenKind::RBrace) => None,
            _ => Some(Box::new(self.parse_expr(0)?)),
        };
        self.expect_semicolon()?;
        Ok(Stmt::Return { value })
    }

    fn parse_type(&mut self) -> Result<Type, AzError> {
        match self.peek_kind() {
            Some(TokenKind::Ident(name)) => {
                let name = name.clone();
                self.advance();
                Ok(Type::Name(name))
            }
            _ => Err(self.err(format!("expected type, found {}", self.peek_kind().unwrap()))),
        }
    }

    fn parse_expr(&mut self, min_bp: u8) -> Result<Expr, AzError> {
        let mut lhs = match self.peek_kind() {
            Some(TokenKind::Int(n)) => { self.advance(); Expr::Int(n) }
            Some(TokenKind::Float(n)) => { self.advance(); Expr::Float(n) }
            Some(TokenKind::String(s)) => { self.advance(); Expr::String(s) }
            Some(TokenKind::Char(c)) => { self.advance(); Expr::Char(c) }
            Some(TokenKind::True) => { self.advance(); Expr::Bool(true) }
            Some(TokenKind::False) => { self.advance(); Expr::Bool(false) }
            Some(TokenKind::Null) => { self.advance(); Expr::Null }
            Some(TokenKind::Self_) => { self.advance(); Expr::Self_ }
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
            Some(TokenKind::LBracket) => self.parse_array()?,
            Some(TokenKind::If) => self.parse_if_expr()?,
            Some(TokenKind::While) => self.parse_while_expr()?,
            Some(TokenKind::Match) => self.parse_match_expr()?,
            Some(TokenKind::Minus) | Some(TokenKind::Not) => {
                let op = match self.peek_kind() { Some(TokenKind::Minus) => UnOp::Neg, _ => UnOp::Not };
                self.advance();
                let ((), r_bp) = prefix_binding_power(op);
                let operand = self.parse_expr(r_bp)?;
                Expr::Unary { op, operand: Box::new(operand) }
            }
            Some(ref other) => return Err(self.err(format!("unexpected token in expression: {}", other))),
            None => return Err(self.err("unexpected end of input in expression")),
        };

        loop {
            match self.peek_kind() {
                Some(TokenKind::LParen) => {
                    // Check if LHS is a method call (ident(..)), otherwise regular call
                    self.advance();
                    let mut args = Vec::new();
                    loop {
                        match self.peek_kind() {
                            Some(TokenKind::RParen) | None => break,
                            Some(TokenKind::Comma) => { self.advance(); }
                            _ => { args.push(self.parse_expr(0)?); }
                        }
                    }
                    self.expect(TokenKind::RParen, "expected ')' after arguments")?;
                    lhs = Expr::Call { callee: Box::new(lhs), args };
                }
                Some(TokenKind::DotDot) => {
                    self.advance();
                    let rhs = self.parse_expr(9)?; // low binding power
                    lhs = Expr::Range { start: Box::new(lhs), end: Box::new(rhs) };
                }
                Some(TokenKind::LBracket) => {
                    self.advance();
                    let index = self.parse_expr(0)?;
                    self.expect(TokenKind::RBracket, "expected ']' after index")?;
                    lhs = Expr::Index { obj: Box::new(lhs), index: Box::new(index) };
                }
                Some(TokenKind::Dot) => {
                    self.advance();
                    let field = match self.peek_kind() {
                        Some(TokenKind::Ident(name)) => { 
                            let name = name.clone(); self.advance(); name 
                        }
                        _ => return Err(self.err("expected field or method name after '.'")),
                    };
                    // Check if it's a method call
                    if self.peek_kind() == Some(TokenKind::LParen) {
                        self.advance();
                        let mut args = Vec::new();
                        loop {
                            match self.peek_kind() {
                                Some(TokenKind::RParen) | None => break,
                                Some(TokenKind::Comma) => { self.advance(); }
                                _ => { args.push(self.parse_expr(0)?); }
                            }
                        }
                        self.expect(TokenKind::RParen, "expected ')' after method arguments")?;
                        lhs = Expr::MethodCall { obj: Box::new(lhs), method: field, args };
                    } else {
                        lhs = Expr::FieldAccess { obj: Box::new(lhs), field };
                    }
                }
                Some(ref op_kind) if is_binop(op_kind) => {
                    let op = token_to_binop(op_kind.clone()).unwrap();
                    let (l_bp, r_bp) = infix_binding_power(op);
                    if l_bp < min_bp { break; }
                    self.advance();
                    let rhs = self.parse_expr(r_bp)?;
                    lhs = Expr::Binary { left: Box::new(lhs), op, right: Box::new(rhs) };
                }
                _ => break,
            }
        }

        Ok(lhs)
    }

    fn parse_array(&mut self) -> Result<Expr, AzError> {
        self.advance(); // consume '['
        let mut elements = Vec::new();
        loop {
            match self.peek_kind() {
                Some(TokenKind::RBracket) | None => break,
                Some(TokenKind::Comma) => { self.advance(); }
                _ => { elements.push(self.parse_expr(0)?); }
            }
        }
        self.expect(TokenKind::RBracket, "expected ']'")?;
        Ok(Expr::Array(elements))
    }

    fn parse_block(&mut self) -> Result<Expr, AzError> {
        self.expect(TokenKind::LBrace, "expected '{'")?;
        let mut statements = Vec::new();
        loop {
            match self.peek_kind() {
                Some(TokenKind::RBrace) | None => break,
                _ => { statements.push(self.parse_stmt()?); }
            }
        }
        self.expect(TokenKind::RBrace, "expected '}'")?;
        Ok(Expr::Block(statements))
    }

    fn parse_if_expr(&mut self) -> Result<Expr, AzError> {
        self.advance();
        let condition = self.parse_expr(0)?;
        let then_branch = self.parse_block()?;
        let else_branch = if self.peek_kind() == Some(TokenKind::Else) {
            self.advance();
            if self.peek_kind() == Some(TokenKind::If) {
                Some(Box::new(self.parse_if_expr()?))
            } else {
                Some(Box::new(self.parse_block()?))
            }
        } else { None };
        Ok(Expr::If { condition: Box::new(condition), then_branch: Box::new(then_branch), else_branch })
    }

    fn parse_match_expr(&mut self) -> Result<Expr, AzError> {
        self.advance();
        let value = self.parse_expr(0)?;
        self.expect(TokenKind::LBrace, "expected '{' after match value")?;
        let mut arms = Vec::new();
        loop {
            match self.peek_kind() {
                Some(TokenKind::RBrace) | None => break,
                _ => {
                    let pattern = self.parse_pattern()?;
                    self.expect(TokenKind::FatArrow, "expected '=>' after pattern")?;
                    let body = self.parse_expr(0)?;
                    arms.push(MatchArm { pattern, body: Box::new(body) });
                    // optional comma/semicolon between arms
                    if self.peek_kind() == Some(TokenKind::Comma) || self.peek_kind() == Some(TokenKind::Semicolon) {
                        self.advance();
                    }
                }
            }
        }
        self.expect(TokenKind::RBrace, "expected '}' after match arms")?;
        Ok(Expr::Match { value: Box::new(value), arms })
    }

    fn parse_pattern(&mut self) -> Result<Pattern, AzError> {
        match self.peek_kind() {
            Some(TokenKind::Int(n)) => { self.advance(); Ok(Pattern::Int(n)) }
            Some(TokenKind::True) => { self.advance(); Ok(Pattern::Bool(true)) }
            Some(TokenKind::False) => { self.advance(); Ok(Pattern::Bool(false)) }
            Some(TokenKind::String(s)) => { self.advance(); Ok(Pattern::String(s.clone())) }
            Some(TokenKind::Ident(name)) => {
                let name = name.clone();
                self.advance();
                if name == "_" {
                    return Ok(Pattern::Wildcard);
                }
                // Check for Enum.Variant pattern
                if self.peek_kind() == Some(TokenKind::Dot) {
                    self.advance(); // consume '.'
                    let variant = match self.peek_kind() {
                        Some(TokenKind::Ident(v)) => { let v = v.clone(); self.advance(); v }
                        _ => return Err(self.err("expected variant name after '.' in pattern")),
                    };
                    let bindings = if self.peek_kind() == Some(TokenKind::LParen) {
                        self.advance();
                        let mut bs = Vec::new();
                        loop {
                            match self.peek_kind() {
                                Some(TokenKind::RParen) | None => break,
                                Some(TokenKind::Comma) => { self.advance(); }
                                _ => {
                                    match self.peek_kind() {
                                        Some(TokenKind::Ident(b)) => { let b = b.clone(); self.advance(); bs.push(b); }
                                        Some(TokenKind::Self_) => { self.advance(); bs.push("self".to_string()); }
                                        _ => return Err(self.err("expected identifier in pattern binding")),
                                    }
                                }
                            }
                        }
                        self.expect(TokenKind::RParen, "expected ')' after pattern bindings")?;
                        bs
                    } else { Vec::new() };
                    Ok(Pattern::EnumVariant { enum_name: Some(name), variant, bindings })
                } else {
                    // Simple ident pattern (binds value)
                    Ok(Pattern::Ident(name))
                }
            }
            Some(TokenKind::Self_) => {
                self.advance();
                // Check for Enum.Variant
                if self.peek_kind() == Some(TokenKind::Dot) {
                    self.advance();
                    let variant = match self.peek_kind() {
                        Some(TokenKind::Ident(v)) => { let v = v.clone(); self.advance(); v }
                        _ => return Err(self.err("expected variant name")),
                    };
                    Ok(Pattern::EnumVariant { enum_name: Some("self".to_string()), variant, bindings: vec![] })
                } else {
                    Ok(Pattern::Ident("self".to_string()))
                }
            }
            _ => Err(self.err("expected pattern")),
        }
    }

    fn parse_while_expr(&mut self) -> Result<Expr, AzError> {
        self.advance();
        let condition = self.parse_expr(0)?;
        let body = self.parse_block()?;
        Ok(Expr::While { condition: Box::new(condition), body: Box::new(body) })
    }

    fn peek_kind(&self) -> Option<TokenKind> {
        self.tokens.get(self.pos).map(|t| t.kind.clone())
    }

    fn current_span(&self) -> Span {
        self.tokens.get(self.pos).map_or(Span::new(0, 0, 0, 0), |t| t.span)
    }

    fn advance(&mut self) { self.pos += 1; }

    fn is_eof(&self) -> bool { matches!(self.peek_kind(), Some(TokenKind::EOF) | None) }

    fn expect(&mut self, expected: TokenKind, msg: &str) -> Result<(), AzError> {
        match self.peek_kind() {
            Some(ref kind) if *kind == expected => { self.advance(); Ok(()) }
            Some(ref other) => Err(self.err(format!("{}: expected {}, found {}", msg, expected, other))),
            None => Err(self.err(format!("{}: expected {}, found EOF", msg, expected))),
        }
    }

    fn expect_semicolon(&mut self) -> Result<(), AzError> {
        if self.peek_kind() == Some(TokenKind::Semicolon) { self.advance(); }
        Ok(())
    }

    fn parse_ident(&mut self) -> Result<Ident, AzError> {
        match self.peek_kind() {
            Some(TokenKind::Ident(name)) => {
                let span = self.current_span();
                let ident = Ident { name: name.clone(), span };
                self.advance();
                Ok(ident)
            }
            Some(ref other) => Err(self.err(format!("expected identifier, found {}", other))),
            None => Err(self.err("expected identifier, found EOF")),
        }
    }
}

fn prefix_binding_power(op: UnOp) -> ((), u8) {
    match op { UnOp::Neg => ((), 9), UnOp::Not => ((), 9) }
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
