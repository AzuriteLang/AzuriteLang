use azurite_lexer::{Lexer, TokenKind};

#[test]
fn test_empty() {
    let tokens = Lexer::new("").tokenize().unwrap();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].kind, TokenKind::EOF);
}

#[test]
fn test_whitespace_only() {
    let tokens = Lexer::new("   \n  \t  ").tokenize().unwrap();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].kind, TokenKind::EOF);
}

#[test]
fn test_integers() {
    let tokens = Lexer::new("42 0 100").tokenize().unwrap();
    assert_eq!(tokens[0].kind, TokenKind::Int(42));
    assert_eq!(tokens[1].kind, TokenKind::Int(0));
    assert_eq!(tokens[2].kind, TokenKind::Int(100));
}

#[test]
fn test_integer_large() {
    let tokens = Lexer::new("9999999999999").tokenize().unwrap();
    assert_eq!(tokens[0].kind, TokenKind::Int(9999999999999));
}

#[test]
fn test_integer_negative_as_prefix() {
    let tokens = Lexer::new("-42").tokenize().unwrap();
    assert_eq!(tokens[0].kind, TokenKind::Minus);
    assert_eq!(tokens[1].kind, TokenKind::Int(42));
}

#[test]
fn test_integer_multiple_zeros() {
    let tokens = Lexer::new("0 0 000").tokenize().unwrap();
    assert_eq!(tokens[0].kind, TokenKind::Int(0));
    assert_eq!(tokens[1].kind, TokenKind::Int(0));
    assert_eq!(tokens[2].kind, TokenKind::Int(0));
}

#[test]
fn test_floats() {
    let tokens = Lexer::new("3.14 0.5").tokenize().unwrap();
    assert_eq!(tokens[0].kind, TokenKind::Float(3.14));
    assert_eq!(tokens[1].kind, TokenKind::Float(0.5));
}

#[test]
fn test_float_start_with_dot() {
    let tokens = Lexer::new("0.5 .5").tokenize().unwrap();
    assert_eq!(tokens[0].kind, TokenKind::Float(0.5));
    assert_eq!(tokens[1].kind, TokenKind::Dot);
    assert_eq!(tokens[2].kind, TokenKind::Int(5));
}

#[test]
fn test_float_trailing_dot_is_not_float() {
    let tokens = Lexer::new("5.").tokenize().unwrap();
    assert_eq!(tokens[0].kind, TokenKind::Int(5));
    assert_eq!(tokens[1].kind, TokenKind::Dot);
}

#[test]
fn test_float_negative_as_prefix() {
    let tokens = Lexer::new("-3.14").tokenize().unwrap();
    assert_eq!(tokens[0].kind, TokenKind::Minus);
    assert_eq!(tokens[1].kind, TokenKind::Float(3.14));
}
