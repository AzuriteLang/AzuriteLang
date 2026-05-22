use azurite_lexer::{Lexer, TokenKind};
use azurite_parser::ast::*;
use azurite_parser::Parser;

fn parse(src: &str) -> (Vec<azurite_lexer::Token>, Program) {
    let tokens = Lexer::new(src).tokenize().unwrap();
    let prog = Parser::new(tokens.clone()).parse_program().unwrap();
    (tokens, prog)
}

macro_rules! assert_func {
    ($stmt:expr, $name:expr, $nparams:expr) => {
        match &$stmt {
            Stmt::Func { name, params, .. } => {
                assert_eq!(name.name, $name);
                assert_eq!(params.len(), $nparams);
            }
            _ => panic!("expected Stmt::Func"),
        }
    };
}

#[test]
fn test_hello_world() {
    let (tokens, prog) = parse("func main() { let msg = \"Hello!\" print(msg) }");
    assert!(tokens.len() > 5);
    assert_eq!(prog.statements.len(), 1);
    assert_func!(prog.statements[0], "main", 0);
}

#[test]
fn test_arithmetic() {
    let (_, prog) = parse("func calc(a: int, b: int) -> int { return (a + b) * 2 }");
    assert_func!(prog.statements[0], "calc", 2);
}

#[test]
fn test_control_flow() {
    let src = "func classify(x: int) -> string { if x > 0 { return \"pos\" } else { return \"neg\" } }";
    let (_, prog) = parse(src);
    assert_func!(prog.statements[0], "classify", 1);
}

#[test]
fn test_while_loop() {
    let src = "func sum_to(n: int) -> int { let i = 0 while i < n { i = i + 1 } return i }";
    let (_, prog) = parse(src);
    assert_func!(prog.statements[0], "sum_to", 1);
}

#[test]
fn test_empty_program() {
    let (tokens, prog) = parse("");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].kind, TokenKind::EOF);
    assert!(prog.statements.is_empty());
}

#[test]
fn test_only_comment() {
    let (tokens, prog) = parse("// just a comment\n");
    assert_eq!(tokens.len(), 1);
    assert!(prog.statements.is_empty());
}

#[test]
fn test_mixed_decls() {
    let (_, prog) = parse("let x = 10 func f() {} return x");
    assert_eq!(prog.statements.len(), 3);
}

#[test]
fn test_deep_nesting() {
    let (_, prog) = parse("{{{{{ let x = 1 }}}}}");
    let mut current = &prog.statements[0];
    for _ in 0..5 {
        match current {
            Stmt::Expr(Expr::Block(stmts)) => { current = &stmts[0]; }
            _ => panic!("expected block"),
        }
    }
    match current {
        Stmt::Let { name, value, .. } => {
            assert_eq!(name.name, "x");
            assert_eq!(value.as_ref(), &Expr::Int(1));
        }
        _ => panic!("expected let"),
    }
}
