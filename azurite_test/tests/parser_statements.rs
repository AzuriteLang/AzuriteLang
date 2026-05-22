use azurite_lexer::Lexer;
use azurite_parser::ast::*;
use azurite_parser::Parser;

fn parse(src: &str) -> Program {
    let tokens = Lexer::new(src).tokenize().unwrap();
    Parser::new(tokens).parse_program().unwrap()
}

#[test]
fn test_let_simple() {
    let prog = parse("let x = 42");
    match &prog.statements[0] {
        Stmt::Let { name, type_annotation, value } => {
            assert_eq!(name.name, "x");
            assert!(type_annotation.is_none());
            assert!(matches!(value.as_ref(), Expr::Int(42)));
        }
        _ => panic!("expected Stmt::Let"),
    }
}

#[test]
fn test_let_with_type() {
    let prog = parse("let x: int = 10");
    match &prog.statements[0] {
        Stmt::Let { name, type_annotation, .. } => {
            assert_eq!(name.name, "x");
            assert!(type_annotation.is_some());
        }
        _ => panic!("expected Stmt::Let"),
    }
}

#[test]
fn test_let_bool() {
    let prog = parse("let a = true");
    match &prog.statements[0] {
        Stmt::Let { name, value, .. } => {
            assert_eq!(name.name, "a");
            assert_eq!(value.as_ref(), &Expr::Bool(true));
        }
        _ => panic!("expected Stmt::Let"),
    }
}

#[test]
fn test_let_string() {
    let prog = parse(r#"let msg = "hello""#);
    match &prog.statements[0] {
        Stmt::Let { name, value, .. } => {
            assert_eq!(name.name, "msg");
            assert_eq!(value.as_ref(), &Expr::String("hello".to_string()));
        }
        _ => panic!("expected Stmt::Let"),
    }
}

#[test]
fn test_let_float() {
    let prog = parse("let pi = 3.14");
    match &prog.statements[0] {
        Stmt::Let { value, .. } => {
            assert!(matches!(value.as_ref(), Expr::Float(f) if (*f - 3.14).abs() < 1e-10));
        }
        _ => panic!("expected Stmt::Let"),
    }
}

#[test]
fn test_return_value() {
    let prog = parse("return 42");
    match &prog.statements[0] {
        Stmt::Return { value } => assert!(value.is_some()),
        _ => panic!("expected Stmt::Return"),
    }
}

#[test]
fn test_return_empty() {
    let prog = parse("return");
    match &prog.statements[0] {
        Stmt::Return { value } => assert!(value.is_none()),
        _ => panic!("expected Stmt::Return"),
    }
}

#[test]
fn test_multiple_stmts() {
    let prog = parse("let a = 1 let b = 2 return a + b");
    assert_eq!(prog.statements.len(), 3);
}

#[test]
fn test_empty_block() {
    let prog = parse("{}");
    match &prog.statements[0] {
        Stmt::Expr(Expr::Block(stmts)) => assert!(stmts.is_empty()),
        _ => panic!("expected block"),
    }
}

#[test]
fn test_block_multiple_stmts() {
    let prog = parse("{ let x = 1 let y = 2 x + y }");
    match &prog.statements[0] {
        Stmt::Expr(Expr::Block(stmts)) => assert_eq!(stmts.len(), 3),
        _ => panic!("expected block"),
    }
}
