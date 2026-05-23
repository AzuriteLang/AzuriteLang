use azurite_errors::AzError;
use azurite_lexer::TokenKind;
use crate::ast::*;
use crate::parser::Parser;

pub fn parse_type(p: &mut Parser) -> Result<Type, AzError> {
    match p.peek_kind() {
        Some(TokenKind::Ident(name)) => {
            let name = name.to_string();
            p.advance();
            if p.peek_kind() == Some(TokenKind::Less) {
                p.advance();
                let mut params = Vec::new();
                loop {
                    match p.peek_kind() {
                        Some(TokenKind::Greater) | None => break,
                        Some(TokenKind::Comma) => { p.advance(); }
                        _ => { params.push(parse_type(p)?); }
                    }
                }
                p.expect(TokenKind::Greater, "'>'")?;
                Ok(Type::Generic { name, params })
            } else {
                Ok(Type::Name(name))
            }
        }
        _ => Err(p.err(format!("expected type, found {}", p.peek_kind().unwrap()))),
    }
}
