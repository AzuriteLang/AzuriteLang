use azurite_lexer::Lexer;

#[test]
fn test_span_simple() {
    let tokens = Lexer::new("42").tokenize().unwrap();
    assert_eq!(tokens[0].span.start, 0);
    assert_eq!(tokens[0].span.end, 2);
    assert_eq!(tokens[0].span.line, 1);
    assert_eq!(tokens[0].span.column, 1);
}

#[test]
fn test_span_multiline() {
    let tokens = Lexer::new("\n\n42").tokenize().unwrap();
    assert_eq!(tokens[0].span.line, 3);
    assert_eq!(tokens[0].span.column, 1);
}

#[test]
fn test_span_multiple_tokens() {
    let tokens = Lexer::new("let x = 1").tokenize().unwrap();
    assert_eq!(tokens[0].span.start, 0);
    assert_eq!(tokens[1].span.start, 4);
    assert_eq!(tokens[2].span.start, 6);
    assert_eq!(tokens[3].span.start, 8);
}
