use azurite_lexer::{Lexer, TokenKind};
use azurite_parser::ast::*;
use azurite_test::parse_prog;

#[test]
fn test_class_declaration() {
    let prog = parse_prog("class Person { name: string age: int }");
    match &prog.statements[0] {
        Stmt::Class { name, fields, methods, .. } => {
            assert_eq!(name.name, "Person");
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].name.name, "name");
            assert_eq!(fields[1].name.name, "age");
            assert!(methods.is_empty());
        }
        _ => panic!("expected Stmt::Class"),
    }
}

#[test]
fn test_class_with_method() {
    let prog = parse_prog("class Foo { x: int func f(self) {} }");
    match &prog.statements[0] {
        Stmt::Class { name, fields, methods, .. } => {
            assert_eq!(name.name, "Foo");
            assert_eq!(fields.len(), 1);
            assert_eq!(methods.len(), 1);
            match &methods[0] {
                Stmt::Func { name: mname, params, .. } => {
                    assert_eq!(mname.name, "f");
                    assert_eq!(params.len(), 1);
                    assert_eq!(params[0].name.name, "self");
                }
                _ => panic!("expected Stmt::Func in method"),
            }
        }
        _ => panic!("expected Stmt::Class"),
    }
}

#[test]
fn test_class_parses_after_other_stmts() {
    let prog = parse_prog("let x = 1 class Empty {} return x");
    assert_eq!(prog.statements.len(), 3);
    match &prog.statements[1] {
        Stmt::Class { name, .. } => assert_eq!(name.name, "Empty"),
        _ => panic!("expected class"),
    }
}

#[test]
fn test_class_multiple_methods() {
    let prog = parse_prog("class C { func a() {} func b() {} }");
    match &prog.statements[0] {
        Stmt::Class { methods, .. } => assert_eq!(methods.len(), 2),
        _ => panic!("expected class"),
    }
}

#[test]
fn test_self_keyword() {
    let prog = parse_prog("func f(self) {}");
    match &prog.statements[0] {
        Stmt::Func { params, .. } => {
            assert_eq!(params[0].name.name, "self");
        }
        _ => panic!("expected func"),
    }
}

#[test]
fn test_method_call_syntax() {
    let prog = parse_prog("let p = Person.new(\"A\", 30) p.greet()");
    assert_eq!(prog.statements.len(), 2);
}

#[test]
fn test_field_access_syntax() {
    let prog = parse_prog("let name = p.name");
    match &prog.statements[0] {
        Stmt::Let { value, .. } => {
            assert!(matches!(value.as_ref(), Expr::FieldAccess { .. }));
        }
        _ => panic!("expected let"),
    }
}

#[test]
fn test_lexer_self_keyword() {
    let tokens = Lexer::new("self").tokenize().unwrap();
    assert_eq!(tokens[0].kind, TokenKind::Self_);
}

#[test]
fn test_lexer_class_keyword() {
    let tokens = Lexer::new("class").tokenize().unwrap();
    assert_eq!(tokens[0].kind, TokenKind::Class);
}

#[test]
fn test_lexer_self_not_identifier() {
    let tokens = Lexer::new("self selfie").tokenize().unwrap();
    assert_eq!(tokens[0].kind, TokenKind::Self_);
    assert_eq!(tokens[1].kind, TokenKind::Ident("selfie".into()));
}
