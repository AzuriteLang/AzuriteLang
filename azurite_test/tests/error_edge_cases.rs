use azurite_lexer::Lexer;
use azurite_parser::Parser;

fn parse(src: &str) -> Result<azurite_parser::ast::Program, String> {
    let tokens = Lexer::new(src).tokenize().map_err(|e| e)?;
    Parser::new(tokens).parse_program().map_err(|e| e.to_string())
}

// --- Parse error cases ---

#[test]
fn test_err_empty_let() {
    assert!(parse("let").is_err());
}

#[test]
fn test_err_empty_func() {
    assert!(parse("func").is_err());
}

#[test]
fn test_err_missing_rbrace() {
    assert!(parse("{ let x = 1").is_err());
}

#[test]
fn test_err_missing_rparen() {
    assert!(parse("foo(1, 2").is_err());
}

// Trailing commas are accepted by the parser
#[test]
fn test_trailing_comma_call() {
    assert!(parse("foo(1,)").is_ok());
}

#[test]
fn test_err_empty_class() {
    assert!(parse("class").is_err());
}

#[test]
fn test_err_let_no_assign() {
    assert!(parse("let x").is_err());
}

#[test]
fn test_err_func_no_body() {
    assert!(parse("func f()").is_err());
}

#[test]
fn test_err_invalid_type() {
    assert!(parse("let x: 123 = 1").is_err());
}

#[test]
fn test_err_operator_only() {
    assert!(parse("+").is_err());
}

#[test]
fn test_err_double_dot() {
    assert!(parse("..5").is_err());
}

#[test]
fn test_err_dot_not_field() {
    assert!(parse("x.123").is_err());
}

#[test]
fn test_err_empty_match() {
    assert!(parse("match x {}").is_ok());
}

#[test]
fn test_err_for_no_in() {
    assert!(parse("for i").is_err());
}

#[test]
fn test_err_for_no_range() {
    assert!(parse("for i in {}").is_err());
}

#[test]
fn test_err_empty_parens() {
    assert!(parse("()").is_err());
}

#[test]
fn test_err_unclosed_bracket() {
    assert!(parse("arr[").is_err());
}

#[test]
fn test_err_class_field_no_colon() {
    assert!(parse("class X { x int }").is_err());
}

#[test]
fn test_err_enum_no_brace() {
    assert!(parse("enum X").is_err());
}

// --- Lexer error cases ---

#[test]
fn test_lex_err_unterminated_string() {
    assert!(Lexer::new("\"hello").tokenize().is_err());
}

#[test]
fn test_lex_err_unterminated_char() {
    assert!(Lexer::new("'a").tokenize().is_err());
}

#[test]
fn test_lex_err_empty_char() {
    assert!(Lexer::new("''").tokenize().is_err());
}

#[test]
fn test_lex_err_invalid_escape() {
    assert!(Lexer::new(r#""\x""#).tokenize().is_err());
}

#[test]
fn test_lex_err_bad_escape_char() {
    assert!(Lexer::new("'\\x'").tokenize().is_err());
}

// --- Edge: weird but valid ---

#[test]
fn test_valid_weird_indent() {
    assert!(parse("let\nx\n=\n1").is_ok());
}

#[test]
fn test_valid_multiline_string() {
    assert!(parse("\"hello\nworld\"").is_ok());
}

#[test]
fn test_valid_many_semicolons() {
    // Single extra semicolon is OK (optional semicolons)
    assert!(parse("let x = 1; let y = 2").is_ok());
}

#[test]
fn test_valid_nested_blocks_deep() {
    let src = "{{{{{{ let x = 1 }}}}}}";
    assert!(parse(src).is_ok());
}

#[test]
fn test_valid_class_method_with_self() {
    assert!(parse("class C { func f(self) {} }").is_ok());
}

#[test]
fn test_valid_func_with_all_types() {
    assert!(parse("func f(a: int, b: float, c: string, d: bool) {}").is_ok());
}

#[test]
fn test_valid_match_with_complex_body() {
    assert!(parse("match x { 1 => { let y = 2 print(y) } _ => 0 }").is_ok());
}

#[test]
fn test_valid_generic_return() {
    assert!(parse("func f() -> Box<int> { return 0 }").is_ok());
}

#[test]
fn test_valid_import() {
    assert!(parse(r#"import "math.az""#).is_ok());
}

#[test]
fn test_valid_while_true() {
    assert!(parse("while true { print(1) }").is_ok());
}

#[test]
fn test_valid_for_range_expr() {
    assert!(parse("for i in a..b { print(i) }").is_ok());
}

#[test]
fn test_valid_chained_assign() {
    assert!(parse("x = y = 5").is_ok());
}

#[test]
fn test_valid_if_else_if_else() {
    assert!(parse("if a { 1 } else if b { 2 } else { 3 }").is_ok());
}

#[test]
fn test_valid_nested_if() {
    assert!(parse("if a { if b { 1 } else { 2 } } else { 3 }").is_ok());
}

#[test]
fn test_valid_while_with_if_inside() {
    assert!(parse("while x < 10 { if x == 5 { print(5) } x = x + 1 }").is_ok());
}
