use azurite_lexer::Lexer;
use azurite_parser::ast::*;
use azurite_parser::Parser;

fn parse(src: &str) -> Program {
    let tokens = Lexer::new(src).tokenize().unwrap();
    Parser::new(tokens).parse_program().unwrap()
}

#[test]
fn test_literal_int() { assert_eq!(parse("42").statements[0], Stmt::Expr(Expr::Int(42))); }
#[test]
fn test_literal_float() { assert_eq!(parse("3.14").statements[0], Stmt::Expr(Expr::Float(3.14))); }
#[test]
fn test_literal_string() { assert_eq!(parse(r#""hello""#).statements[0], Stmt::Expr(Expr::String("hello".to_string()))); }
#[test]
fn test_literal_char() { assert_eq!(parse("'x'").statements[0], Stmt::Expr(Expr::Char('x'))); }
#[test]
fn test_literal_bool_true() { assert_eq!(parse("true").statements[0], Stmt::Expr(Expr::Bool(true))); }
#[test]
fn test_literal_bool_false() { assert_eq!(parse("false").statements[0], Stmt::Expr(Expr::Bool(false))); }
#[test]
fn test_literal_null() { assert_eq!(parse("null").statements[0], Stmt::Expr(Expr::Null)); }

#[test]
fn test_complex_condition() {
    let prog = parse("x > 0 and x < 10 or x == 42");
    match &prog.statements[0] {
        Stmt::Expr(Expr::Binary { op, .. }) => assert_eq!(*op, BinOp::Or),
        _ => panic!("expected or"),
    }
}
