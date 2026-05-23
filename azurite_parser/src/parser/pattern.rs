use azurite_errors::AzError;
use azurite_lexer::TokenKind;
use crate::ast::*;
use crate::parser::Parser;

pub fn parse_pattern(p: &mut Parser) -> Result<Pattern, AzError> {
    match p.peek_kind() {
        Some(TokenKind::Int(n)) => { p.advance(); Ok(Pattern::Int(n)) }
        Some(TokenKind::True) => { p.advance(); Ok(Pattern::Bool(true)) }
        Some(TokenKind::False) => { p.advance(); Ok(Pattern::Bool(false)) }
        Some(TokenKind::String(s)) => { p.advance(); Ok(Pattern::String(s.to_string())) }
        Some(TokenKind::Ident(name)) => {
            let name = name.to_string();
            p.advance();
            if name == "_" { return Ok(Pattern::Wildcard); }
            if p.peek_kind() == Some(TokenKind::Dot) {
                p.advance();
                let variant = match p.peek_kind() {
                    Some(TokenKind::Ident(v)) => { let v = v.to_string(); p.advance(); v }
                    _ => return Err(p.err("expected variant name")),
                };
                let bindings = if p.peek_kind() == Some(TokenKind::LParen) {
                    p.advance();
                    let mut bs = Vec::new();
                    loop {
                        match p.peek_kind() {
                            Some(TokenKind::RParen) | None => break,
                            Some(TokenKind::Comma) => { p.advance(); }
                            _ => {
                                match p.peek_kind() {
                                    Some(TokenKind::Ident(b)) => { let b = b.to_string(); p.advance(); bs.push(b); }
                                    Some(TokenKind::Self_) => { p.advance(); bs.push("self".to_string()); }
                                    _ => return Err(p.err("expected identifier in pattern")),
                                }
                            }
                        }
                    }
                    p.expect(TokenKind::RParen, "')'")?;
                    bs
                } else { Vec::new() };
                Ok(Pattern::EnumVariant { enum_name: Some(name), variant, bindings })
            } else {
                Ok(Pattern::Ident(name))
            }
        }
        Some(TokenKind::Self_) => {
            p.advance();
            if p.peek_kind() == Some(TokenKind::Dot) {
                p.advance();
                let variant = match p.peek_kind() {
                    Some(TokenKind::Ident(v)) => { let v = v.to_string(); p.advance(); v }
                    _ => return Err(p.err("expected variant name")),
                };
                Ok(Pattern::EnumVariant { enum_name: Some("self".to_string()), variant, bindings: vec![] })
            } else {
                Ok(Pattern::Ident("self".to_string()))
            }
        }
        _ => Err(p.err("expected pattern")),
    }
}
