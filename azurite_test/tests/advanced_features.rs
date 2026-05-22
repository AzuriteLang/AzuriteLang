use azurite_lexer::{Lexer, TokenKind};
use azurite_parser::ast::*;
use azurite_parser::Parser;

fn parse(src: &str) -> Program {
    let tokens = Lexer::new(src).tokenize().unwrap();
    Parser::new(tokens).parse_program().unwrap()
}

// --- Classes avance ---

#[test]
fn test_class_empty() {
    let prog = parse("class Empty {}");
    match &prog.statements[0] {
        Stmt::Class { name, fields, methods, .. } => {
            assert_eq!(name.name, "Empty");
            assert!(fields.is_empty());
            assert!(methods.is_empty());
        }
        _ => panic!("expected class"),
    }
}

#[test]
fn test_class_fields_only() {
    let prog = parse("class Data { x: int y: float z: string }");
    match &prog.statements[0] {
        Stmt::Class { fields, .. } => assert_eq!(fields.len(), 3),
        _ => panic!("expected class"),
    }
}

#[test]
fn test_class_methods_only() {
    let prog = parse("class Util { func a() {} func b() {} func c() {} }");
    match &prog.statements[0] {
        Stmt::Class { methods, .. } => assert_eq!(methods.len(), 3),
        _ => panic!("expected class"),
    }
}

#[test]
fn test_class_field_types() {
    let prog = parse("class T { a: int b: bool c: float d: string }");
    match &prog.statements[0] {
        Stmt::Class { fields, .. } => {
            assert!(matches!(&fields[0].type_, Type::Name(n) if n == "int"));
            assert!(matches!(&fields[1].type_, Type::Name(n) if n == "bool"));
            assert!(matches!(&fields[2].type_, Type::Name(n) if n == "float"));
            assert!(matches!(&fields[3].type_, Type::Name(n) if n == "string"));
        }
        _ => panic!("expected class"),
    }
}

#[test]
fn test_class_self_param() {
    let prog = parse("class X { func f(self, x: int) {} }");
    match &prog.statements[0] {
        Stmt::Class { methods, .. } => {
            match &methods[0] {
                Stmt::Func { params, .. } => {
                    assert_eq!(params[0].name.name, "self");
                    assert_eq!(params[1].name.name, "x");
                }
                _ => panic!("expected func"),
            }
        }
        _ => panic!("expected class"),
    }
}

// --- Heritage ---

#[test]
fn test_inheritance_simple() {
    let prog = parse("class A {} class B : A {}");
    assert_eq!(prog.statements.len(), 2);
    match &prog.statements[1] {
        Stmt::Class { name, parent, .. } => {
            assert_eq!(name.name, "B");
            assert!(parent.is_some());
        }
        _ => panic!("expected class"),
    }
}

#[test]
fn test_inheritance_chain() {
    let prog = parse("class A {} class B : A {} class C : B {}");
    assert_eq!(prog.statements.len(), 3);
}

// --- Generics ---

#[test]
fn test_generic_class() {
    let prog = parse("class Box<T> { value: T }");
    match &prog.statements[0] {
        Stmt::Class { name, type_params, .. } => {
            assert_eq!(name.name, "Box");
            assert_eq!(type_params.len(), 1);
            assert_eq!(type_params[0], "T");
        }
        _ => panic!("expected class"),
    }
}

#[test]
fn test_generic_multi_params() {
    let prog = parse("class Pair<A, B> { first: A second: B }");
    match &prog.statements[0] {
        Stmt::Class { type_params, .. } => assert_eq!(type_params.len(), 2),
        _ => panic!("expected class"),
    }
}

#[test]
fn test_generic_type_usage() {
    let prog = parse("let x: Box<int> = 0");
    match &prog.statements[0] {
        Stmt::Let { type_annotation, .. } => {
            assert!(type_annotation.is_some());
            match type_annotation.as_ref().unwrap() {
                Type::Generic { name, params } => {
                    assert_eq!(name, "Box");
                    assert_eq!(params.len(), 1);
                }
                _ => panic!("expected generic type"),
            }
        }
        _ => panic!("expected let"),
    }
}

#[test]
fn test_generic_nested() {
    let prog = parse("let x: Box< Pair<int, string> > = 0");
    match &prog.statements[0] {
        Stmt::Let { type_annotation, .. } => {
            match type_annotation.as_ref().unwrap() {
                Type::Generic { name, params } => {
                    assert_eq!(name, "Box");
                    match &params[0] {
                        Type::Generic { name: n, .. } => {
                            assert_eq!(n, "Pair");
                        }
                        _ => panic!("expected nested generic"),
                    }
                }
                _ => panic!("expected generic"),
            }
        }
        _ => panic!("expected let"),
    }
}

// --- Enums avance ---

#[test]
fn test_enum_empty() {
    let prog = parse("enum E {}");
    match &prog.statements[0] {
        Stmt::Enum { name, variants } => {
            assert_eq!(name.name, "E");
            assert!(variants.is_empty());
        }
        _ => panic!("expected enum"),
    }
}

#[test]
fn test_enum_complex_variants() {
    let prog = parse("enum Expr { Int(int), Float(float), Str(string), Pair(int, string) }");
    match &prog.statements[0] {
        Stmt::Enum { variants, .. } => {
            assert_eq!(variants.len(), 4);
            assert_eq!(variants[0].types.len(), 1);
            assert_eq!(variants[3].types.len(), 2);
        }
        _ => panic!("expected enum"),
    }
}

// --- Pattern matching ---

#[test]
fn test_match_literal() {
    let prog = parse("match x { 42 => true }");
    match &prog.statements[0] {
        Stmt::Expr(Expr::Match { arms, .. }) => {
            assert_eq!(arms[0].pattern, Pattern::Int(42));
        }
        _ => panic!("expected match"),
    }
}

#[test]
fn test_match_string() {
    let prog = parse(r#"match x { "hello" => 1 }"#);
    match &prog.statements[0] {
        Stmt::Expr(Expr::Match { arms, .. }) => {
            assert_eq!(arms[0].pattern, Pattern::String("hello".to_string()));
        }
        _ => panic!("expected match"),
    }
}

#[test]
fn test_match_multi_arms() {
    let prog = parse("match x { 1 => 2 3 => 4 5 => 6 _ => 0 }");
    match &prog.statements[0] {
        Stmt::Expr(Expr::Match { arms, .. }) => assert_eq!(arms.len(), 4),
        _ => panic!("expected match"),
    }
}

#[test]
fn test_match_wildcard() {
    let prog = parse("match x { _ => 99 }");
    match &prog.statements[0] {
        Stmt::Expr(Expr::Match { arms, .. }) => {
            assert_eq!(arms[0].pattern, Pattern::Wildcard);
        }
        _ => panic!("expected match"),
    }
}

// --- For loops ---

#[test]
fn test_for_range_zero() {
    let prog = parse("for i in 0..0 { print(i) }");
    assert_eq!(prog.statements.len(), 1);
}

#[test]
fn test_for_range_negative() {
    let prog = parse("for i in -5..5 { print(i) }");
    assert_eq!(prog.statements.len(), 1);
}

#[test]
fn test_for_empty_body() {
    let prog = parse("for i in 0..10 {}");
    assert_eq!(prog.statements.len(), 1);
}

// --- Expressions complexes ---

#[test]
fn test_nested_arithmetic() {
    let prog = parse("((1 + 2) * (3 - 4)) / 5");
    assert!(matches!(&prog.statements[0], Stmt::Expr(Expr::Binary { .. })));
}

#[test]
fn test_deeply_nested_parens() {
    let prog = parse("((((((42))))))");
    match &prog.statements[0] {
        Stmt::Expr(Expr::Int(42)) => {}
        _ => panic!("expected int"),
    }
}

#[test]
fn test_chained_method_calls() {
    let prog = parse("a.b().c().d()");
    assert!(matches!(&prog.statements[0], Stmt::Expr(Expr::MethodCall { .. })));
}

#[test]
fn test_nested_index() {
    let prog = parse("matrix[i][j]");
    assert!(matches!(&prog.statements[0], Stmt::Expr(Expr::Index { .. })));
}

#[test]
fn test_array_of_arrays() {
    let prog = parse("[[1, 2], [3, 4]]");
    match &prog.statements[0] {
        Stmt::Expr(Expr::Array(outer)) => {
            assert_eq!(outer.len(), 2);
            assert!(matches!(&outer[0], Expr::Array(inner) if inner.len() == 2));
        }
        _ => panic!("expected array"),
    }
}

#[test]
fn test_empty_array() {
    let prog = parse("[]");
    match &prog.statements[0] {
        Stmt::Expr(Expr::Array(v)) => assert!(v.is_empty()),
        _ => panic!("expected array"),
    }
}

// --- String operations ---

#[test]
fn test_string_concat_multi() {
    let prog = parse(r#""a" + "b" + "c""#);
    assert!(matches!(&prog.statements[0], Stmt::Expr(Expr::Binary { .. })));
}

#[test]
fn test_string_escape() {
    let tokens = Lexer::new(r#""\n\t\r\\\0""#).tokenize().unwrap();
    assert_eq!(tokens[0].kind, TokenKind::String("\n\t\r\\\0".to_string()));
}

// --- Comment edge cases ---

#[test]
fn test_comment_only_file() {
    let tokens = Lexer::new("// just a comment\n// another\n").tokenize().unwrap();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].kind, TokenKind::EOF);
}

#[test]
fn test_mixed_comments() {
    let prog = parse("42 // inline\n/* block */ 99");
    assert_eq!(prog.statements.len(), 2);
}

// --- Operator edge cases ---

#[test]
fn test_bitwise_ops() {
    let src = "1 & 2 | 3 ^ 4 << 5 >> 6";
    let prog = parse(src);
    assert_eq!(prog.statements.len(), 1);
}

#[test]
fn test_logical_ops() {
    let src = "true && false || true and false or true";
    let prog = parse(src);
    assert_eq!(prog.statements.len(), 1);
}

#[test]
fn test_comparison_chain() {
    let src = "a < b == c > d <= e >= f";
    let prog = parse(src);
    assert_eq!(prog.statements.len(), 1);
}

// --- Multi-statement programs ---

#[test]
fn test_program_with_all_stmt_types() {
    let src = r#"
        let x: int = 42
        func f() { return x }
        if true { print(1) }
        while false { print(2) }
        for i in 0..5 { print(i) }
        return 0
    "#;
    let prog = parse(src);
    assert_eq!(prog.statements.len(), 6);
}

#[test]
fn test_program_ordering() {
    let src = "let a = 1 func f() {} let b = 2";
    let prog = parse(src);
    assert_eq!(prog.statements.len(), 3);
    assert!(matches!(&prog.statements[0], Stmt::Let { .. }));
    assert!(matches!(&prog.statements[1], Stmt::Func { .. }));
    assert!(matches!(&prog.statements[2], Stmt::Let { .. }));
}

// --- Edge case: keywords as identifiers ---

#[test]
fn test_keyword_prefix_variable() {
    let tokens = Lexer::new("letty if_else").tokenize().unwrap();
    assert!(matches!(tokens[0].kind, TokenKind::Ident(_)));
    assert!(matches!(tokens[1].kind, TokenKind::Ident(_)));
}

// --- Edge case: numbers ---

#[test]
fn test_many_numbers() {
    let tokens = Lexer::new("0 1 2 3 4 5 6 7 8 9 10 100 1000").tokenize().unwrap();
    assert_eq!(tokens.len(), 14);
}

#[test]
fn test_many_floats() {
    let tokens = Lexer::new("0.5 1.5 2.5 3.14159").tokenize().unwrap();
    assert_eq!(tokens.len(), 5);
}
