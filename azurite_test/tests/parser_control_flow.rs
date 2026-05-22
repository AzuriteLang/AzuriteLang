use azurite_lexer::Lexer;
use azurite_parser::ast::*;
use azurite_parser::Parser;

fn parse(src: &str) -> Program {
    let tokens = Lexer::new(src).tokenize().unwrap();
    Parser::new(tokens).parse_program().unwrap()
}

#[test]
fn test_if_simple() {
    let prog = parse("if true { 42 }");
    assert_eq!(prog.statements.len(), 1);
}

#[test]
fn test_if_else() {
    let prog = parse("if x { 1 } else { 2 }");
    assert_eq!(prog.statements.len(), 1);
}

#[test]
fn test_if_else_if() {
    let prog = parse("if a { 1 } else if b { 2 } else { 3 }");
    assert_eq!(prog.statements.len(), 1);
}

#[test]
fn test_while_simple() {
    let prog = parse("while x < 10 { x = x + 1 }");
    assert_eq!(prog.statements.len(), 1);
}
