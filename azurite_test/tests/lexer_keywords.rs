use azurite_lexer::{Lexer, TokenKind};

#[test]
fn test_keywords() {
    let tokens = Lexer::new("let func if else while for return import struct enum true false null and or not")
        .tokenize().unwrap();
    assert_eq!(tokens[0].kind, TokenKind::Let);
    assert_eq!(tokens[1].kind, TokenKind::Func);
    assert_eq!(tokens[2].kind, TokenKind::If);
    assert_eq!(tokens[3].kind, TokenKind::Else);
    assert_eq!(tokens[4].kind, TokenKind::While);
    assert_eq!(tokens[5].kind, TokenKind::For);
    assert_eq!(tokens[6].kind, TokenKind::Return);
    assert_eq!(tokens[7].kind, TokenKind::Import);
    assert_eq!(tokens[8].kind, TokenKind::Struct);
    assert_eq!(tokens[9].kind, TokenKind::Enum);
    assert_eq!(tokens[10].kind, TokenKind::True);
    assert_eq!(tokens[11].kind, TokenKind::False);
    assert_eq!(tokens[12].kind, TokenKind::Null);
    assert_eq!(tokens[13].kind, TokenKind::And);
    assert_eq!(tokens[14].kind, TokenKind::Or);
    assert_eq!(tokens[15].kind, TokenKind::Not);
}

#[test]
fn test_keywords_are_not_identifiers() {
    let tokens = Lexer::new("let letty").tokenize().unwrap();
    assert_eq!(tokens[0].kind, TokenKind::Let);
    assert_eq!(tokens[1].kind, TokenKind::Ident("letty".into()));
}

#[test]
fn test_booleans_and_null() {
    let tokens = Lexer::new("true false null").tokenize().unwrap();
    assert_eq!(tokens[0].kind, TokenKind::True);
    assert_eq!(tokens[1].kind, TokenKind::False);
    assert_eq!(tokens[2].kind, TokenKind::Null);
}

#[test]
fn test_logical_operators_keywords() {
    let tokens = Lexer::new("and or not").tokenize().unwrap();
    assert_eq!(tokens[0].kind, TokenKind::And);
    assert_eq!(tokens[1].kind, TokenKind::Or);
    assert_eq!(tokens[2].kind, TokenKind::Not);
}

#[test]
fn test_hash_not_comment() {
    let tokens = Lexer::new("# not a comment").tokenize().unwrap();
    assert_eq!(tokens[0].kind, TokenKind::Hash);
}
