use azurite_lexer::Lexer;
use azurite_parser::Parser;

fn expect_err(src: &str) {
    let tokens = Lexer::new(src).tokenize().unwrap();
    let result = std::panic::catch_unwind(|| {
        Parser::new(tokens).parse_program().unwrap();
    });
    assert!(result.is_err(), "expected parse error for: {}", src);
}

#[test]
fn test_error_unexpected_token() { expect_err("let 42 = 1"); }
#[test]
fn test_error_missing_rparen() { expect_err("foo("); }
#[test]
fn test_error_missing_rbrace() { expect_err("{ let x = 1 "); }
#[test]
fn test_error_empty_expr() { expect_err("()"); }
#[test]
fn test_error_bad_type() { expect_err("let x: = 1"); }
#[test]
fn test_error_invalid_func() { expect_err("func 123() {}"); }
#[test]
fn test_error_eof_in_expr() { expect_err("1 +"); }
