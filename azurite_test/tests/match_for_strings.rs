use azurite_lexer::Lexer;
use azurite_parser::ast::*;
use azurite_parser::Parser;

fn parse(src: &str) -> Program {
    let tokens = Lexer::new(src).tokenize().unwrap();
    Parser::new(tokens).parse_program().unwrap()
}

#[test]
fn test_for_loop() {
    let prog = parse("for i in 0..10 { print(i) }");
    assert_eq!(prog.statements.len(), 1);
    match &prog.statements[0] {
        Stmt::For { name, .. } => assert_eq!(name.name, "i"),
        _ => panic!("expected For"),
    }
}

#[test]
fn test_match_simple() {
    let prog = parse("match x { 1 => true _ => false }");
    match &prog.statements[0] {
        Stmt::Expr(Expr::Match { arms, .. }) => {
            assert_eq!(arms.len(), 2);
            assert_eq!(arms[0].pattern, Pattern::Int(1));
            assert_eq!(arms[1].pattern, Pattern::Wildcard);
        }
        _ => panic!("expected Match"),
    }
}

#[test]
fn test_match_enum_variant() {
    let prog = parse("match opt { Option.Some(v) => v Option.None => 0 }");
    match &prog.statements[0] {
        Stmt::Expr(Expr::Match { value: _v, arms }) => {
            assert_eq!(arms.len(), 2);
            match &arms[0].pattern {
                Pattern::EnumVariant { enum_name, variant, bindings } => {
                    assert_eq!(enum_name.as_ref().unwrap(), "Option");
                    assert_eq!(variant, "Some");
                    assert_eq!(bindings.len(), 1);
                }
                _ => panic!("expected EnumVariant pattern"),
            }
        }
        _ => panic!("expected Match"),
    }
}

#[test]
fn test_range() {
    let prog = parse("0..10");
    match &prog.statements[0] {
        Stmt::Expr(Expr::Range { start, end }) => {
            assert_eq!(**start, Expr::Int(0));
            assert_eq!(**end, Expr::Int(10));
        }
        _ => panic!("expected Range"),
    }
}

#[test]
fn test_for_range_combined() {
    let prog = parse("for i in 0..5 { print(i) }");
    assert_eq!(prog.statements.len(), 1);
}

#[test]
fn test_keywords() {
    let tokens = Lexer::new("match for").tokenize().unwrap();
    assert_eq!(tokens[0].kind, azurite_lexer::TokenKind::Match);
    assert_eq!(tokens[1].kind, azurite_lexer::TokenKind::For);
}

#[test]
fn test_dotdot_token() {
    let tokens = Lexer::new("..").tokenize().unwrap();
    assert_eq!(tokens[0].kind, azurite_lexer::TokenKind::DotDot);
}

#[test]
fn test_wildcard_pattern() {
    let prog = parse("match x { _ => 0 }");
    match &prog.statements[0] {
        Stmt::Expr(Expr::Match { arms, .. }) => {
            assert_eq!(arms[0].pattern, Pattern::Wildcard);
        }
        _ => panic!("expected match"),
    }
}
