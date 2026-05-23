use azurite_lexer::{Lexer, TokenKind};

#[test]
fn test_unexpected_char() {
    let tokens = Lexer::new("~").tokenize().unwrap();
    assert!(matches!(tokens[0].kind, TokenKind::Error(_)));
}

#[test]
fn test_unexpected_chars() {
    let tokens = Lexer::new("@` ?").tokenize().unwrap();
    assert!(matches!(tokens[0].kind, TokenKind::Error(_)));
    assert!(matches!(tokens[1].kind, TokenKind::Error(_)));
}

#[test]
fn test_string_unterminated() {
    assert!(Lexer::new(r#""hello"#).tokenize().is_err());
}

#[test]
fn test_string_invalid_escape() {
    assert!(Lexer::new(r#""\z""#).tokenize().is_err());
}

#[test]
fn test_char_unterminated() {
    assert!(Lexer::new("'a").tokenize().is_err());
}

#[test]
fn test_char_empty() {
    assert!(Lexer::new("''").tokenize().is_err());
}

#[test]
fn test_char_invalid_escape() {
    assert!(Lexer::new("'\\x'").tokenize().is_err());
}
