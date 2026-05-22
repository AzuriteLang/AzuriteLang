use azurite_lexer::Lexer;
use azurite_parser::ast::*;
use azurite_parser::Parser;

fn parse(src: &str) -> Program {
    let tokens = Lexer::new(src).tokenize().unwrap();
    Parser::new(tokens).parse_program().unwrap()
}

#[test]
fn test_precedence_mul_over_add() {
    let prog = parse("1 + 2 * 3");
    match &prog.statements[0] {
        Stmt::Expr(Expr::Binary { left, op, right }) => {
            assert_eq!(*op, BinOp::Add);
            assert_eq!(**left, Expr::Int(1));
            assert!(matches!(right.as_ref(), Expr::Binary { op: BinOp::Mul, .. }));
        }
        _ => panic!("expected add"),
    }
}

#[test]
fn test_precedence_parens() {
    let prog = parse("(1 + 2) * 3");
    match &prog.statements[0] {
        Stmt::Expr(Expr::Binary { left, op, right }) => {
            assert_eq!(*op, BinOp::Mul);
            assert!(matches!(left.as_ref(), Expr::Binary { op: BinOp::Add, .. }));
            assert_eq!(**right, Expr::Int(3));
        }
        _ => panic!("expected mul"),
    }
}

#[test]
fn test_precedence_comparison_over_and() {
    let prog = parse("a < b && c > d");
    match &prog.statements[0] {
        Stmt::Expr(Expr::Binary { left, op, right }) => {
            assert_eq!(*op, BinOp::And);
            assert!(matches!(left.as_ref(), Expr::Binary { op: BinOp::Lt, .. }));
            assert!(matches!(right.as_ref(), Expr::Binary { op: BinOp::Gt, .. }));
        }
        _ => panic!("expected and"),
    }
}

#[test]
fn test_precedence_assign() {
    let prog = parse("x = a + b");
    match &prog.statements[0] {
        Stmt::Expr(Expr::Binary { left, op, right }) => {
            assert_eq!(*op, BinOp::Assign);
            assert!(matches!(left.as_ref(), Expr::Ident(_)));
            assert!(matches!(right.as_ref(), Expr::Binary { op: BinOp::Add, .. }));
        }
        _ => panic!("expected assign"),
    }
}

#[test]
fn test_precedence_multiple_add() {
    let prog = parse("1 + 2 + 3");
    match &prog.statements[0] {
        Stmt::Expr(Expr::Binary { left, op: _op, right }) => {
            assert!(matches!(left.as_ref(), Expr::Binary { op: BinOp::Add, .. }));
            assert_eq!(**right, Expr::Int(3));
        }
        _ => panic!("expected binary"),
    }
}
