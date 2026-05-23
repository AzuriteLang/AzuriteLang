use azurite_errors::AzError;
use azurite_lexer::TokenKind;
use crate::ast::*;
use crate::parser::{self, Parser};

pub fn parse_expr(p: &mut Parser, min_bp: u8) -> Result<Expr, AzError> {
    let mut lhs = match p.peek_kind() {
        Some(TokenKind::Int(n)) => { p.advance(); Expr::Int(n) }
        Some(TokenKind::Float(n)) => { p.advance(); Expr::Float(n) }
        Some(TokenKind::String(s)) => { p.advance(); Expr::String(s.to_string()) }
        Some(TokenKind::Char(c)) => { p.advance(); Expr::Char(c) }
        Some(TokenKind::True) => { p.advance(); Expr::Bool(true) }
        Some(TokenKind::False) => { p.advance(); Expr::Bool(false) }
        Some(TokenKind::Null) => { p.advance(); Expr::Null }
        Some(TokenKind::Self_) => { p.advance(); Expr::Self_ }
        Some(TokenKind::Super) => { p.advance(); Expr::Super }
        Some(TokenKind::Ident(name)) => {
            let ident = Ident { name: name.to_string(), span: p.current_span() };
            p.advance();
            Expr::Ident(ident)
        }
        Some(TokenKind::LParen) => { p.advance(); let e = parse_expr(p, 0)?; if p.peek_kind() == Some(TokenKind::Comma) { let mut elems = vec![e]; while p.peek_kind() == Some(TokenKind::Comma) { p.advance(); if p.peek_kind() == Some(TokenKind::RParen) { break; } elems.push(parse_expr(p, 0)?); } p.expect(TokenKind::RParen, "')'")?; Expr::Tuple(elems) } else { p.expect(TokenKind::RParen, "')'")?; e } }
        Some(TokenKind::LBrace) => parse_block(p)?,
        Some(TokenKind::LBracket) => parse_array(p)?,
        Some(TokenKind::If) => parse_if(p)?,
        Some(TokenKind::While) => parse_while(p)?,
        Some(TokenKind::Match) => parse_match(p)?,
        Some(TokenKind::Minus) | Some(TokenKind::Not) => {
            let op = match p.peek_kind() { Some(TokenKind::Minus) => UnOp::Neg, _ => UnOp::Not };
            p.advance();
            let ((), r_bp) = parser::prefix_binding_power(op);
            Expr::Unary { op, operand: Box::new(parse_expr(p, r_bp)?) }
        }
        Some(ref other) => return Err(p.err(format!("unexpected token: {}", other))),
        None => return Err(p.err("unexpected end of input")),
    };

    loop {
        match p.peek_kind() {
            Some(TokenKind::LParen) => {
                p.advance();
                let args = parse_call_args(p)?;
                p.expect(TokenKind::RParen, "')'")?;
                lhs = Expr::Call { callee: Box::new(lhs), args };
            }
            Some(TokenKind::LBracket) => {
                p.advance();
                let index = parse_expr(p, 0)?;
                p.expect(TokenKind::RBracket, "']'")?;
                lhs = Expr::Index { obj: Box::new(lhs), index: Box::new(index) };
            }
            Some(TokenKind::Dot) | Some(TokenKind::QuestionDot) => {
                let null_safe = p.peek_kind() == Some(TokenKind::QuestionDot);
                p.advance();
                let field = match p.peek_kind() {
                    Some(TokenKind::Ident(name)) => { let n = name.to_string(); p.advance(); n }
                    _ => return Err(p.err("expected field name after '.'")),
                };
                if p.peek_kind() == Some(TokenKind::LParen) {
                    p.advance();
                    let args = parse_call_args(p)?;
                    p.expect(TokenKind::RParen, "')'")?;
                    lhs = Expr::MethodCall { obj: Box::new(lhs), method: field, args, null_safe };
                } else {
                    lhs = Expr::FieldAccess { obj: Box::new(lhs), field, null_safe };
                }
            }
            Some(TokenKind::DotDot) => {
                p.advance();
                let rhs = parse_expr(p, 9)?;
                lhs = Expr::Range { start: Box::new(lhs), end: Box::new(rhs) };
            }
            Some(op_kind) if parser::is_binop(&op_kind) => {
                let op = parser::token_to_binop(op_kind.clone()).unwrap();
                let (l_bp, r_bp) = parser::infix_binding_power(op);
                if l_bp < min_bp { break; }
                p.advance();
                let rhs = parse_expr(p, r_bp)?;
                // Compound assignment desugar: x += 1 → x = x + 1
                if let Some(compound_op) = parser::token_to_compound_binop(op_kind.clone()) {
                    let left_clone = lhs.clone();
                    lhs = Expr::Binary {
                        left: Box::new(left_clone),
                        op: BinOp::Assign,
                        right: Box::new(Expr::Binary {
                            left: Box::new(lhs),
                            op: compound_op,
                            right: Box::new(rhs),
                        }),
                    };
                    continue;
                } else if parser::is_comparison(op) {
                    if let Some(next_kind) = p.peek_kind() {
                        if let Some(next_op) = parser::token_to_binop(next_kind) {
                            if parser::is_comparison(next_op) {
                                let mid = rhs.clone();
                                p.advance();
                                let rhs2 = parse_expr(p, r_bp)?;
                                let second = Expr::Binary { left: Box::new(mid), op: next_op, right: Box::new(rhs2) };
                                lhs = Expr::Binary { left: Box::new(Expr::Binary { left: Box::new(lhs), op, right: Box::new(rhs) }), op: BinOp::And, right: Box::new(second) };
                                continue;
                            }
                        }
                    }
                }
                lhs = Expr::Binary { left: Box::new(lhs), op, right: Box::new(rhs) };
            }
            _ => break,
        }
    }
    Ok(lhs)
}

pub fn parse_block(p: &mut Parser) -> Result<Expr, AzError> {
    p.expect(TokenKind::LBrace, "'{'")?;
    let mut stmts = Vec::new();
    loop {
        match p.peek_kind() {
            Some(TokenKind::RBrace) | None => break,
            _ => { stmts.push(super::stmt::parse_stmt(p)?); }
        }
    }
    p.expect(TokenKind::RBrace, "'}'")?;
    Ok(Expr::Block(stmts))
}

fn parse_call_args(p: &mut Parser) -> Result<Vec<Expr>, AzError> {
    let mut args = Vec::new();
    loop {
        match p.peek_kind() {
            Some(TokenKind::RParen) | None => break,
            Some(TokenKind::Comma) => { p.advance(); }
            _ => { args.push(parse_expr(p, 0)?); }
        }
    }
    Ok(args)
}

fn parse_array(p: &mut Parser) -> Result<Expr, AzError> {
    p.advance();
    let mut elems = Vec::new();
    loop {
        match p.peek_kind() {
            Some(TokenKind::RBracket) | None => break,
            Some(TokenKind::Comma) => { p.advance(); }
            _ => { elems.push(parse_expr(p, 0)?); }
        }
    }
    p.expect(TokenKind::RBracket, "']'")?;
    Ok(Expr::Array(elems))
}

pub fn parse_if(p: &mut Parser) -> Result<Expr, AzError> {
    p.advance();
    let condition = parse_expr(p, 0)?;
    let then_branch = parse_block(p)?;
    let else_branch = if p.peek_kind() == Some(TokenKind::Else) {
        p.advance();
        Some(Box::new(if p.peek_kind() == Some(TokenKind::If) { parse_if(p)? } else { parse_block(p)? }))
    } else { None };
    Ok(Expr::If { condition: Box::new(condition), then_branch: Box::new(then_branch), else_branch })
}

pub fn parse_while(p: &mut Parser) -> Result<Expr, AzError> {
    p.advance();
    let condition = parse_expr(p, 0)?;
    let body = parse_block(p)?;
    Ok(Expr::While { condition: Box::new(condition), body: Box::new(body) })
}

fn parse_match(p: &mut Parser) -> Result<Expr, AzError> {
    p.advance();
    let value = parse_expr(p, 0)?;
    p.expect(TokenKind::LBrace, "'{'")?;
    let mut arms = Vec::new();
    loop {
        match p.peek_kind() {
            Some(TokenKind::RBrace) | None => break,
            _ => {
                let pattern = super::pattern::parse_pattern(p)?;
                p.expect(TokenKind::FatArrow, "'=>'")?;
                let body = parse_expr(p, 0)?;
                arms.push(MatchArm { pattern, body: Box::new(body) });
                if matches!(p.peek_kind(), Some(TokenKind::Comma | TokenKind::Semicolon)) { p.advance(); }
            }
        }
    }
    p.expect(TokenKind::RBrace, "'}'")?;
    Ok(Expr::Match { value: Box::new(value), arms })
}
