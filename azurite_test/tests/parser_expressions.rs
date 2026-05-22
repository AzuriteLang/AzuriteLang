use azurite_lexer::Lexer;
use azurite_parser::ast::*;
use azurite_parser::Parser;

fn parse(src: &str) -> Program {
    let tokens = Lexer::new(src).tokenize().unwrap();
    Parser::new(tokens).parse_program().unwrap()
}

fn check_binop(src: &str, expected: BinOp) {
    let prog = parse(src);
    match &prog.statements[0] {
        Stmt::Expr(Expr::Binary { op, .. }) => assert_eq!(*op, expected),
        _ => panic!("expected binary {:?}", expected),
    }
}

#[test]
fn test_identifier() {
    let prog = parse("myVar");
    match &prog.statements[0] {
        Stmt::Expr(Expr::Ident(ident)) => assert_eq!(ident.name, "myVar"),
        _ => panic!("expected ident"),
    }
}

#[test]
fn test_binary_add() { check_binop("1 + 2", BinOp::Add); }
#[test]
fn test_binary_sub() { check_binop("10 - 3", BinOp::Sub); }
#[test]
fn test_binary_mul() { check_binop("4 * 5", BinOp::Mul); }
#[test]
fn test_binary_div() { check_binop("10 / 2", BinOp::Div); }
#[test]
fn test_binary_mod() { check_binop("10 % 3", BinOp::Mod); }
#[test]
fn test_binary_eq() { check_binop("a == b", BinOp::Eq); }
#[test]
fn test_binary_neq() { check_binop("a != b", BinOp::Neq); }
#[test]
fn test_binary_lt() { check_binop("a < b", BinOp::Lt); }
#[test]
fn test_binary_gt() { check_binop("a > b", BinOp::Gt); }
#[test]
fn test_binary_le() { check_binop("a <= b", BinOp::Le); }
#[test]
fn test_binary_ge() { check_binop("a >= b", BinOp::Ge); }
#[test]
fn test_binary_and() { check_binop("a && b", BinOp::And); }
#[test]
fn test_binary_or() { check_binop("a || b", BinOp::Or); }
#[test]
fn test_binary_logical_keywords() { check_binop("a and b", BinOp::And); }
#[test]
fn test_binary_assign() { check_binop("x = 42", BinOp::Assign); }

#[test]
fn test_unary_neg() {
    let prog = parse("-42");
    match &prog.statements[0] {
        Stmt::Expr(Expr::Unary { op, operand }) => {
            assert_eq!(*op, UnOp::Neg);
            assert_eq!(**operand, Expr::Int(42));
        }
        _ => panic!("expected unary neg"),
    }
}

#[test]
fn test_unary_not() {
    let prog = parse("not true");
    match &prog.statements[0] {
        Stmt::Expr(Expr::Unary { op, operand }) => {
            assert_eq!(*op, UnOp::Not);
            assert_eq!(**operand, Expr::Bool(true));
        }
        _ => panic!("expected unary not"),
    }
}

#[test]
fn test_double_neg() {
    let prog = parse("--5");
    match &prog.statements[0] {
        Stmt::Expr(Expr::Unary { op, operand }) => {
            assert_eq!(*op, UnOp::Neg);
            assert!(matches!(operand.as_ref(), Expr::Unary { op: UnOp::Neg, .. }));
        }
        _ => panic!("expected double neg"),
    }
}

#[test]
fn test_call_no_args() {
    let prog = parse("foo()");
    match &prog.statements[0] {
        Stmt::Expr(Expr::Call { args, .. }) => assert!(args.is_empty()),
        _ => panic!("expected call"),
    }
}

#[test]
fn test_call_one_arg() {
    let prog = parse("print(42)");
    match &prog.statements[0] {
        Stmt::Expr(Expr::Call { args, .. }) => {
            assert_eq!(args.len(), 1);
            assert_eq!(args[0], Expr::Int(42));
        }
        _ => panic!("expected call"),
    }
}

#[test]
fn test_call_multi_args() {
    let prog = parse("add(1, 2, 3)");
    match &prog.statements[0] {
        Stmt::Expr(Expr::Call { args, .. }) => assert_eq!(args.len(), 3),
        _ => panic!("expected call"),
    }
}

#[test]
fn test_call_in_expr() {
    let prog = parse("1 + foo(2)");
    match &prog.statements[0] {
        Stmt::Expr(Expr::Binary { right, .. }) => {
            assert!(matches!(right.as_ref(), Expr::Call { .. }));
        }
        _ => panic!("expected binary with call"),
    }
}

#[test]
fn test_nested_calls() {
    let prog = parse("foo(bar(baz()))");
    match &prog.statements[0] {
        Stmt::Expr(Expr::Call { callee, .. }) => {
            assert!(matches!(callee.as_ref(), Expr::Ident(i) if i.name == "foo"));
        }
        _ => panic!("expected nested calls"),
    }
}

#[test]
fn test_nested_binary() {
    let prog = parse("a + b * c - d / e");
    match &prog.statements[0] {
        Stmt::Expr(Expr::Binary { .. }) => {}
        _ => panic!("expected binary"),
    }
}
