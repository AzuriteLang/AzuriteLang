use azurite_lexer::{Lexer, TokenKind};
use azurite_parser::ast::*;
use azurite_parser::Parser;

fn parse_prog(src: &str) -> Program {
    let tokens = Lexer::new(src).tokenize().unwrap();
    Parser::new(tokens).parse_program().unwrap()
}

// --- Enums ---

#[test]
fn test_enum_simple() {
    let prog = parse_prog("enum Color { Red, Green, Blue }");
    match &prog.statements[0] {
        Stmt::Enum { name, variants } => {
            assert_eq!(name.name, "Color");
            assert_eq!(variants.len(), 3);
            assert_eq!(variants[0].name.name, "Red");
            assert_eq!(variants[1].name.name, "Green");
            assert_eq!(variants[2].name.name, "Blue");
        }
        _ => panic!("expected Stmt::Enum"),
    }
}

#[test]
fn test_enum_with_data() {
    let prog = parse_prog("enum Option { Some(int), None }");
    match &prog.statements[0] {
        Stmt::Enum { name, variants } => {
            assert_eq!(name.name, "Option");
            assert_eq!(variants.len(), 2);
            assert_eq!(variants[0].name.name, "Some");
            assert_eq!(variants[0].types.len(), 1);
            assert_eq!(variants[1].name.name, "None");
            assert_eq!(variants[1].types.len(), 0);
        }
        _ => panic!("expected Stmt::Enum"),
    }
}

#[test]
fn test_enum_multiple_data() {
    let prog = parse_prog("enum Either { Left(int, string), Right(float) }");
    match &prog.statements[0] {
        Stmt::Enum { variants, .. } => {
            assert_eq!(variants[0].name.name, "Left");
            assert_eq!(variants[0].types.len(), 2);
            assert_eq!(variants[1].name.name, "Right");
            assert_eq!(variants[1].types.len(), 1);
        }
        _ => panic!("expected enum"),
    }
}

// --- Arrays ---

#[test]
fn test_array_empty() {
    let prog = parse_prog("let a = []");
    match &prog.statements[0] {
        Stmt::Let { value, .. } => {
            assert!(matches!(value.as_ref(), Expr::Array(v) if v.is_empty()));
        }
        _ => panic!("expected let with array"),
    }
}

#[test]
fn test_array_literal() {
    let prog = parse_prog("let a = [1, 2, 3]");
    match &prog.statements[0] {
        Stmt::Let { value, .. } => {
            match value.as_ref() {
                Expr::Array(v) => assert_eq!(v.len(), 3),
                _ => panic!("expected array"),
            }
        }
        _ => panic!("expected let"),
    }
}

#[test]
fn test_index_expr() {
    let prog = parse_prog("let x = arr[0]");
    match &prog.statements[0] {
        Stmt::Let { value, .. } => {
            assert!(matches!(value.as_ref(), Expr::Index { .. }));
        }
        _ => panic!("expected index"),
    }
}

#[test]
fn test_nested_index() {
    let prog = parse_prog("let x = matrix[i][j]");
    match &prog.statements[0] {
        Stmt::Let { value, .. } => {
            assert!(matches!(value.as_ref(), Expr::Index { .. }));
        }
        _ => panic!("expected index"),
    }
}

// --- Lexer ---

#[test]
fn test_lexer_bracket_tokens() {
    let tokens = Lexer::new("[ ]").tokenize().unwrap();
    assert_eq!(tokens[0].kind, TokenKind::LBracket);
    assert_eq!(tokens[1].kind, TokenKind::RBracket);
}

#[test]
fn test_lexer_enum_keyword() {
    let tokens = Lexer::new("enum").tokenize().unwrap();
    assert_eq!(tokens[0].kind, TokenKind::Enum);
}
