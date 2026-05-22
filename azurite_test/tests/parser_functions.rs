use azurite_lexer::Lexer;
use azurite_parser::ast::*;
use azurite_parser::Parser;

fn parse(src: &str) -> Program {
    let tokens = Lexer::new(src).tokenize().unwrap();
    Parser::new(tokens).parse_program().unwrap()
}

#[test]
fn test_func_no_params() {
    let prog = parse("func main() {}");
    match &prog.statements[0] {
        Stmt::Func { name, params, return_type, .. } => {
            assert_eq!(name.name, "main");
            assert!(params.is_empty());
            assert!(return_type.is_none());
        }
        _ => panic!("expected Stmt::Func"),
    }
}

#[test]
fn test_func_with_params() {
    let prog = parse("func add(a: int, b: int) -> int { return a + b }");
    match &prog.statements[0] {
        Stmt::Func { name, params, return_type, .. } => {
            assert_eq!(name.name, "add");
            assert_eq!(params.len(), 2);
            assert_eq!(params[0].name.name, "a");
            assert!(return_type.is_some());
        }
        _ => panic!("expected Stmt::Func"),
    }
}

#[test]
fn test_func_no_return_type() {
    let prog = parse("func greet(name: string) { print(name) }");
    match &prog.statements[0] {
        Stmt::Func { return_type, .. } => assert!(return_type.is_none()),
        _ => panic!("expected Stmt::Func"),
    }
}

#[test]
fn test_func_empty_block() {
    let prog = parse("func nop() {}");
    assert_eq!(prog.statements.len(), 1);
}

#[test]
fn test_full_program() {
    let prog = parse("func main() { let x = 1 if true { print(x) } return 0 }");
    assert_eq!(prog.statements.len(), 1);
    match &prog.statements[0] {
        Stmt::Func { name, .. } => assert_eq!(name.name, "main"),
        _ => panic!("expected func"),
    }
}
