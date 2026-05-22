use azurite_lexer::{Lexer, TokenKind};

#[test]
fn test_identifiers() {
    let tokens = Lexer::new("foo bar _baz my_var123").tokenize().unwrap();
    assert_eq!(tokens[0].kind, TokenKind::Ident("foo".to_string()));
    assert_eq!(tokens[1].kind, TokenKind::Ident("bar".to_string()));
    assert_eq!(tokens[2].kind, TokenKind::Ident("_baz".to_string()));
    assert_eq!(tokens[3].kind, TokenKind::Ident("my_var123".to_string()));
}

#[test]
fn test_identifier_with_numbers() {
    let tokens = Lexer::new("a1 b2_3 _1_2_3").tokenize().unwrap();
    assert_eq!(tokens[0].kind, TokenKind::Ident("a1".to_string()));
    assert_eq!(tokens[1].kind, TokenKind::Ident("b2_3".to_string()));
    assert_eq!(tokens[2].kind, TokenKind::Ident("_1_2_3".to_string()));
}

#[test]
fn test_identifier_uppercase() {
    let tokens = Lexer::new("Foo BAR _Test").tokenize().unwrap();
    assert_eq!(tokens[0].kind, TokenKind::Ident("Foo".to_string()));
    assert_eq!(tokens[1].kind, TokenKind::Ident("BAR".to_string()));
    assert_eq!(tokens[2].kind, TokenKind::Ident("_Test".to_string()));
}

#[test]
fn test_func_declaration() {
    let tokens = Lexer::new("func add(a int, b int) -> int { return a + b }").tokenize().unwrap();
    assert_eq!(tokens[0].kind, TokenKind::Func);
    assert_eq!(tokens[1].kind, TokenKind::Ident("add".to_string()));
    assert_eq!(tokens[16].kind, TokenKind::RBrace);
}

#[test]
fn test_if_expression() {
    let tokens = Lexer::new("if x > 0 { print(\"pos\") } else { print(\"neg\") }").tokenize().unwrap();
    assert_eq!(tokens[0].kind, TokenKind::If);
    assert_eq!(tokens[10].kind, TokenKind::Else);
    assert_eq!(tokens[16].kind, TokenKind::RBrace);
}
