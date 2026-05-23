use azurite_lexer::{Lexer, TokenKind};

#[test]
fn test_strings() {
    let tokens = Lexer::new(r#""hello" "world""#).tokenize().unwrap();
    assert_eq!(tokens[0].kind, TokenKind::String("hello".into()));
    assert_eq!(tokens[1].kind, TokenKind::String("world".into()));
}

#[test]
fn test_string_empty() {
    let tokens = Lexer::new(r#""""#).tokenize().unwrap();
    assert_eq!(tokens[0].kind, TokenKind::String("".into()));
}

#[test]
fn test_string_escape_sequences() {
    let tokens = Lexer::new(r#""\n\t\r\\\"\'\0""#).tokenize().unwrap();
    assert_eq!(tokens[0].kind, TokenKind::String("\n\t\r\\\"\'\0".into()));
}

#[test]
fn test_string_multiline() {
    let tokens = Lexer::new("\"hello\nworld\"").tokenize().unwrap();
    assert_eq!(tokens[0].kind, TokenKind::String("hello\nworld".into()));
}

#[test]
fn test_chars() {
    let tokens = Lexer::new("'a' '\\n'").tokenize().unwrap();
    assert_eq!(tokens[0].kind, TokenKind::Char('a'));
    assert_eq!(tokens[1].kind, TokenKind::Char('\n'));
}

#[test]
fn test_char_escape_all() {
    let tokens = Lexer::new("'\\n' '\\t' '\\r' '\\\\' '\\'' '\\0'").tokenize().unwrap();
    assert_eq!(tokens[0].kind, TokenKind::Char('\n'));
    assert_eq!(tokens[1].kind, TokenKind::Char('\t'));
    assert_eq!(tokens[2].kind, TokenKind::Char('\r'));
    assert_eq!(tokens[3].kind, TokenKind::Char('\\'));
    assert_eq!(tokens[4].kind, TokenKind::Char('\''));
    assert_eq!(tokens[5].kind, TokenKind::Char('\0'));
}
