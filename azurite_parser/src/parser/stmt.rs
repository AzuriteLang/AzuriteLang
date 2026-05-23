use azurite_errors::AzError;
use azurite_lexer::TokenKind;
use crate::ast::*;
use crate::parser::{Parser, expr};

pub fn parse_stmt(p: &mut Parser) -> Result<Stmt, AzError> {
    match p.peek_kind() {
        Some(TokenKind::Let) => parse_let(p),
        Some(TokenKind::Func) => parse_func(p),
        Some(TokenKind::Class) => parse_class(p),
        Some(TokenKind::Enum) => parse_enum(p),
        Some(TokenKind::If) => {
            let expr = expr::parse_if(p)?;
            if let Expr::If { condition, then_branch, else_branch } = expr {
                Ok(Stmt::If { condition, then_branch, else_branch })
            } else { unreachable!() }
        }
        Some(TokenKind::While) => {
            let expr = expr::parse_while(p)?;
            if let Expr::While { condition, body } = expr {
                Ok(Stmt::While { condition, body })
            } else { unreachable!() }
        }
        Some(TokenKind::For) => parse_for(p),
        Some(TokenKind::Import) => parse_import(p),
        Some(TokenKind::Return) => parse_return(p),
        Some(TokenKind::Break) => { p.advance(); p.expect_semicolon()?; Ok(Stmt::Break) }
        Some(TokenKind::Continue) => { p.advance(); p.expect_semicolon()?; Ok(Stmt::Continue) }
        _ => {
            let e = expr::parse_expr(p, 0)?;
            p.expect_semicolon()?;
            Ok(Stmt::Expr(e))
        }
    }
}

fn parse_let(p: &mut Parser) -> Result<Stmt, AzError> {
    p.advance();
    if p.peek_kind() == Some(TokenKind::LParen) {
        p.advance();
        let mut names = Vec::new();
        loop {
            match p.peek_kind() {
                Some(TokenKind::RParen) => { p.advance(); break; }
                Some(TokenKind::Comma) => { p.advance(); }
                Some(TokenKind::Ident(_)) => { names.push(p.parse_ident()?); }
                Some(other) => return Err(p.err(format!("expected identifier or ')', found {}", other))),
                None => return Err(p.err("expected identifier or ')'")),
            }
        }
        p.expect(TokenKind::Assign, "expected '=' in let destructure")?;
        let value = expr::parse_expr(p, 0)?;
        p.expect_semicolon()?;
        Ok(Stmt::Destructure { names, value: Box::new(value) })
    } else {
        let name = p.parse_ident()?;
        let type_annotation = if p.peek_kind() == Some(TokenKind::Colon) {
            p.advance();
            Some(super::type_::parse_type(p)?)
        } else { None };
        p.expect(TokenKind::Assign, "expected '=' in let declaration")?;
        let value = expr::parse_expr(p, 0)?;
        p.expect_semicolon()?;
        Ok(Stmt::Let { name, type_annotation, value: Box::new(value) })
    }
}

fn parse_func(p: &mut Parser) -> Result<Stmt, AzError> {
    p.advance();
    let name = parse_func_name(p)?;
    p.expect(TokenKind::LParen, "expected '(' after function name")?;
    let params = parse_params(p)?;
    p.expect(TokenKind::RParen, "expected ')' after parameters")?;
    let return_type = if p.peek_kind() == Some(TokenKind::Arrow) {
        p.advance();
        Some(super::type_::parse_type(p)?)
    } else { None };
    let body = expr::parse_block(p)?;
    Ok(Stmt::Func { name, params, return_type, body: Box::new(body) })
}

fn parse_func_name(p: &mut Parser) -> Result<Ident, AzError> {
    let span = p.current_span();
    match p.peek_kind() {
        Some(TokenKind::Plus) => { p.advance(); Ok(Ident { name: "+".to_string(), span }) }
        Some(TokenKind::Minus) => { p.advance(); Ok(Ident { name: "-".to_string(), span }) }
        Some(TokenKind::Star) => { p.advance(); Ok(Ident { name: "*".to_string(), span }) }
        Some(TokenKind::Slash) => { p.advance(); Ok(Ident { name: "/".to_string(), span }) }
        Some(TokenKind::Percent) => { p.advance(); Ok(Ident { name: "%".to_string(), span }) }
        Some(TokenKind::Equal) => { p.advance(); Ok(Ident { name: "==".to_string(), span }) }
        Some(TokenKind::NotEqual) => { p.advance(); Ok(Ident { name: "!=".to_string(), span }) }
        Some(TokenKind::Less) => { p.advance(); Ok(Ident { name: "<".to_string(), span }) }
        Some(TokenKind::Greater) => { p.advance(); Ok(Ident { name: ">".to_string(), span }) }
        _ => p.parse_ident(),
    }
}

fn parse_params(p: &mut Parser) -> Result<Vec<Param>, AzError> {
    let mut params = Vec::new();
    loop {
        match p.peek_kind() {
            Some(TokenKind::RParen) | None => break,
            Some(TokenKind::Comma) => { p.advance(); }
            _ => {
                let name = p.parse_ident_or_self()?;
                let type_annotation = if p.peek_kind() == Some(TokenKind::Colon) {
                    p.advance();
                    Some(super::type_::parse_type(p)?)
                } else { None };
                params.push(Param { name, type_annotation });
            }
        }
    }
    Ok(params)
}

fn parse_class(p: &mut Parser) -> Result<Stmt, AzError> {
    p.advance();
    let name = p.parse_ident()?;
    let type_params = if p.peek_kind() == Some(TokenKind::Less) {
        p.advance();
        let mut params = Vec::new();
        loop {
            match p.peek_kind() {
                Some(TokenKind::Greater) | None => break,
                Some(TokenKind::Comma) => { p.advance(); }
                _ => { params.push(p.parse_ident()?.name); }
            }
        }
        p.expect(TokenKind::Greater, "expected '>'")?;
        params
    } else { Vec::new() };
    let parent = if p.peek_kind() == Some(TokenKind::Colon) {
        p.advance();
        Some(Box::new(super::type_::parse_type(p)?))
    } else { None };
    p.expect(TokenKind::LBrace, "expected '{'")?;
    let mut fields = Vec::new();
    let mut methods = Vec::new();
    loop {
        match p.peek_kind() {
            Some(TokenKind::RBrace) | None => break,
            _ => parse_class_member(p, &mut fields, &mut methods)?,
        }
    }
    p.expect(TokenKind::RBrace, "expected '}'")?;
    Ok(Stmt::Class { name, type_params, parent, fields, methods })
}

fn parse_class_member(p: &mut Parser, fields: &mut Vec<ClassField>, methods: &mut Vec<Stmt>) -> Result<(), AzError> {
    match p.peek_kind() {
        Some(TokenKind::Func) => { methods.push(parse_func(p)?); }
        Some(TokenKind::Ident(_)) => {
            let name = p.parse_ident()?;
            if p.peek_kind() == Some(TokenKind::Colon) {
                p.advance();
                let type_ = super::type_::parse_type(p)?;
                p.expect_semicolon()?;
                fields.push(ClassField { name, type_ });
            } else {
                return Err(p.err(format!("expected ':' after field '{}'", name.name)));
            }
        }
        _ => return Err(p.err("unexpected token in class")),
    }
    Ok(())
}

fn parse_enum(p: &mut Parser) -> Result<Stmt, AzError> {
    p.advance();
    let name = p.parse_ident()?;
    p.expect(TokenKind::LBrace, "expected '{'")?;
    let mut variants = Vec::new();
    loop {
        match p.peek_kind() {
            Some(TokenKind::RBrace) | None => break,
            Some(TokenKind::Comma) => { p.advance(); }
            _ => {
                let vname = p.parse_ident()?;
                let types = if p.peek_kind() == Some(TokenKind::LParen) {
                    p.advance();
                    let mut ts = Vec::new();
                    loop {
                        match p.peek_kind() {
                            Some(TokenKind::RParen) | None => break,
                            Some(TokenKind::Comma) => { p.advance(); }
                            _ => { ts.push(super::type_::parse_type(p)?); }
                        }
                    }
                    p.expect(TokenKind::RParen, "expected ')'")?;
                    ts
                } else { Vec::new() };
                variants.push(EnumVariant { name: vname, types });
            }
        }
    }
    p.expect(TokenKind::RBrace, "expected '}'")?;
    Ok(Stmt::Enum { name, variants })
}

fn parse_for(p: &mut Parser) -> Result<Stmt, AzError> {
    p.advance();
    let name = p.parse_ident()?;
    match p.peek_kind() {
        Some(TokenKind::Ident(ref s)) if s.as_ref() == "in" => { p.advance(); }
        Some(other) => return Err(p.err(format!("expected 'in', found {}", other))),
        None => return Err(p.err("expected 'in'")),
    }
    let iterable = expr::parse_expr(p, 0)?;
    let body = expr::parse_block(p)?;
    Ok(Stmt::For { name, iterable: Box::new(iterable), body: Box::new(body) })
}

fn parse_import(p: &mut Parser) -> Result<Stmt, AzError> {
    let span = p.current_span();
    p.advance();
    match p.peek_kind() {
        Some(TokenKind::String(path)) => {
            let path = path.to_string();
            p.advance();
            p.expect_semicolon()?;
            Ok(Stmt::Import { path, span })
        }
        _ => Err(p.err("expected string after 'import'")),
    }
}

fn parse_return(p: &mut Parser) -> Result<Stmt, AzError> {
    p.advance();
    let value = match p.peek_kind() {
        Some(TokenKind::Semicolon) | Some(TokenKind::EOF) | Some(TokenKind::RBrace) => None,
        _ => Some(Box::new(expr::parse_expr(p, 0)?)),
    };
    p.expect_semicolon()?;
    Ok(Stmt::Return { value })
}
