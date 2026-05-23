use azurite_lexer::Lexer;
use azurite_parser::ast::*;
use azurite_test::{check, parse_prog};

// ===== Tuples =====

#[test]
fn test_tuple_expression() {
    assert!(check("func main() { let t = (1, 2, 3) }").is_ok());
}

#[test]
fn test_tuple_two_elements() {
    assert!(check("func main() { let t = (1, \"hello\") }").is_ok());
}

#[test]
fn test_tuple_empty_parens_is_not_tuple() {
    // () must be parsed as empty parens expr, not tuple
    let prog = parse_prog("func main() { let x = (1) }");
    if let Stmt::Func { body, .. } = &prog.statements[0] {
        if let Expr::Block(stmts) = body.as_ref() {
            if let Some(Stmt::Let { value, .. }) = stmts.first() {
                assert!(!matches!(value.as_ref(), Expr::Tuple(_)));
            } else { panic!("expected let"); }
        }
    }
}

#[test]
fn test_destructure_let() {
    assert!(check("func main() { let (a, b) = (1, 2) }").is_ok());
}

#[test]
fn test_destructure_let_assigns() {
    assert!(check("func main() { let (a, b) = (1, 2) print(a) print(b) }").is_ok());
}

#[test]
fn test_multi_return_function() {
    assert!(check("func div_mod(a: int, b: int) -> (int, int) { return (a / b, a % b) } func main() { let (q, r) = div_mod(10, 3) print(q) print(r) }").is_ok());
}

#[test]
fn test_tuple_type_in_fn_sig() {
    let prog = parse_prog("func f() -> (int, string) { return (1, \"x\") }");
    if let Stmt::Func { return_type, .. } = &prog.statements[0] {
        assert!(matches!(return_type, Some(Type::Tuple(_))));
    } else { panic!("expected func"); }
}

// ===== ?. Null-safe operator =====

#[test]
fn test_null_safe_field_access() {
    assert!(check("class C { x: int func new(x: int) {} } func main() { let c = C.new(42) print(c?.x) }").is_ok());
}

#[test]
fn test_null_safe_lexer() {
    let tokens = Lexer::new("a?.b").tokenize().unwrap();
    let kinds: Vec<String> = tokens.iter().map(|t| t.kind.to_string()).collect();
    assert_eq!(kinds, ["a", "?.", "b", "EOF"]);
}

// ===== ..args Varargs =====

#[test]
fn test_varargs_declaration() {
    assert!(check("func f(..args: int) {} func main() { f(1, 2, 3) }").is_ok());
}

#[test]
fn test_varargs_multiple_calls() {
    assert!(check("func f(..args: int) {} func main() { f(1) f(1, 2) f(1, 2, 3) }").is_ok());
}

#[test]
fn test_varargs_with_normal_param() {
    assert!(check("func f(prefix: string, ..values: int) {} func main() { f(\"x\") f(\"y\", 1) f(\"z\", 1, 2) }").is_ok());
}

// ===== String interpolation =====

#[test]
fn test_string_interpolation_basic() {
    assert!(check("func main() { let name = \"world\" print(\"hello \\{name}\") }").is_ok());
}

#[test]
fn test_string_interpolation_multiple() {
    assert!(check("func main() { let a = \"x\" let b = \"y\" print(\"\\{a} and \\{b}\") }").is_ok());
}

#[test]
fn test_string_interpolation_preprocessing() {
    // The preprocessor converts "Hello \{name}" to "Hello " + name
    let result = Lexer::new("func main() { print(\"Hello \\{name}\") }").tokenize().unwrap();
    let kinds: Vec<String> = result.iter().map(|t| t.kind.to_string()).collect();
    let joined = kinds.join(" ");
    assert!(joined.contains("Hello "));
    assert!(joined.contains("+"));
}

// ===== Match exhaustiveness =====

#[test]
fn test_exhaustive_match_all_variants() {
    assert!(check("enum C { A, B } func main() { let x = C.A match x { C.A => {} C.B => {} } }").is_ok());
}

#[test]
fn test_exhaustive_match_with_wildcard() {
    assert!(check("enum C { A, B } func main() { let x = C.A match x { C.A => {} _ => {} } }").is_ok());
}

#[test]
fn test_nonexhaustive_match_fails() {
    assert!(check("enum C { A, B } func main() { let x = C.A match x { C.A => {} } }").is_err());
}

#[test]
fn test_exhaustive_match_three_variants() {
    assert!(check("enum C { A, B, D } func main() { let x = C.A match x { C.A => {} C.B => {} C.D => {} } }").is_ok());
}

// ===== ?. token alone =====
#[test]
fn test_question_token() {
    let tokens = Lexer::new("?").tokenize().unwrap();
    assert_eq!(tokens[0].kind.to_string(), "?");
}

// ===== Compound assignment operators =====

#[test]
fn test_compound_assign_plus() {
    assert!(check("func main() { let x = 1 x += 2 }").is_ok());
}

#[test]
fn test_compound_assign_minus() {
    assert!(check("func main() { let x = 1 x -= 2 }").is_ok());
}

#[test]
fn test_compound_assign_star() {
    assert!(check("func main() { let x = 5 x *= 3 }").is_ok());
}

#[test]
fn test_compound_assign_slash() {
    assert!(check("func main() { let x = 6 x /= 2 }").is_ok());
}

#[test]
fn test_compound_assign_percent() {
    assert!(check("func main() { let x = 7 x %= 3 }").is_ok());
}

#[test]
fn test_compound_assign_lexer_tokens() {
    let tokens = Lexer::new("+= -= *= /= %=").tokenize().unwrap();
    let kinds: Vec<String> = tokens.iter().map(|t| t.kind.to_string()).collect();
    assert_eq!(kinds, ["+=", "-=", "*=", "/=", "%=", "EOF"]);
}

#[test]
fn test_compound_assign_bitwise_lexer() {
    let tokens = Lexer::new("&= |= ^= <<= >>=").tokenize().unwrap();
    let kinds: Vec<String> = tokens.iter().map(|t| t.kind.to_string()).collect();
    assert_eq!(kinds, ["&=", "|=", "^=", "<<=", ">>=", "EOF"]);
}

#[test]
fn test_compound_assign_bitwise() {
    assert!(check("func main() { let x = 1 x &= 3 }").is_ok());
    assert!(check("func main() { let x = 1 x |= 2 }").is_ok());
    assert!(check("func main() { let x = 5 x ^= 3 }").is_ok());
}

#[test]
fn test_compound_assign_shift() {
    assert!(check("func main() { let x = 1 x <<= 2 }").is_ok());
    assert!(check("func main() { let x = 8 x >>= 1 }").is_ok());
}
