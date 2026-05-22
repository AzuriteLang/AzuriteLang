use azurite_lexer::Lexer;
use azurite_parser::Parser;

#[test]
fn test_span_correctness() {
    let tokens = Lexer::new("let x = 42").tokenize().unwrap();
    assert_eq!(tokens[0].span.start, 0);
    assert_eq!(tokens[1].span.start, 4);
    assert_eq!(tokens[2].span.start, 6);
    assert_eq!(tokens[3].span.start, 8);
}

#[test]
fn test_multiline_spans() {
    let tokens = Lexer::new("\n\nlet x = 42").tokenize().unwrap();
    assert_eq!(tokens[0].span.line, 3);
    assert_eq!(tokens[0].span.column, 1);
}

#[test]
fn test_lex_error_propagates() {
    assert!(Lexer::new(r#""unterminated"#).tokenize().is_err());
}

#[test]
fn test_parse_error_propagates() {
    let tokens = Lexer::new("let 42 = 1").tokenize().unwrap();
    assert!(Parser::new(tokens).parse_program().is_err());
}
