use azurite_lexer::Lexer;
use azurite_parser::Parser;
use azurite_checker::Checker;
use azurite_errors::AzError;

fn check(src: &str) -> Result<(), Vec<AzError>> {
    let tokens = Lexer::new(src).tokenize().unwrap();
    let prog = Parser::new(tokens).parse_program().unwrap();
    Checker::new().check_program(&prog)
}

// --- Valid programs ---

#[test]
fn test_check_valid_arithmetic() {
    assert!(check("func f() { let x = 1 + 2 * 3 / 4 - 5 }").is_ok());
}

#[test]
fn test_check_valid_nested_func_calls() {
    assert!(check("func f() { print(42) }").is_ok());
    assert!(check("func f() { let x = sqrt(9.0) }").is_ok());
    assert!(check("func f() { let x = abs(-5) }").is_ok());
}

#[test]
fn test_check_valid_multiple_params() {
    assert!(check("func f(a: int, b: int, c: int) -> int { return a + b + c }").is_ok());
}

#[test]
fn test_check_valid_bool_ops() {
    assert!(check("func f() { let x = true and false or not true }").is_ok());
}

#[test]
fn test_check_valid_comparison() {
    assert!(check("func f() { let x = 1 < 2 and 3 > 4 or 5 == 5 }").is_ok());
}

#[test]
fn test_check_valid_void_func() {
    assert!(check("func f() { return }").is_ok());
}

#[test]
fn test_check_valid_mixed_types() {
    assert!(check("func f() { let a: int = 1 let b: float = 2.5 let c: bool = true }").is_ok());
}

// --- Rejected programs ---

#[test]
fn test_check_reject_bad_arith_types() {
    assert!(check("func f() { let x = true + 1 }").is_err());
}

#[test]
fn test_check_reject_bad_compare() {
    assert!(check("func f() { let x = true < 1 }").is_err());
}

#[test]
fn test_check_reject_bad_and() {
    assert!(check("func f() { let x = 1 and 2 }").is_err());
}

#[test]
fn test_check_reject_bad_or() {
    assert!(check("func f() { let x = 1 or 2 }").is_err());
}

#[test]
fn test_check_reject_string_arith() {
    assert!(check(r#"func f() { let x = "hello" - "world" }"#).is_err());
}

#[test]
fn test_check_reject_bad_negate() {
    assert!(check("func f() { let x = -true }").is_err());
}

#[test]
fn test_check_reject_not_num() {
    assert!(check("func f() { let x = not 42 }").is_err());
}

#[test]
fn test_check_reject_wrong_return() {
    assert!(check("func f() -> int { return \"hello\" }").is_err());
}

#[test]
fn test_check_reject_missing_return() {
    assert!(check("func f() -> int { return }").is_err());
}

#[test]
fn test_check_reject_undefined() {
    assert!(check("func f() { print(undefined_var) }").is_err());
}

#[test]
fn test_check_reject_double_decl() {
    assert!(check("func f() { let x = 1 let x = 2 }").is_err());
}

#[test]
fn test_check_reject_assign_bad_type() {
    assert!(check("func f() { let x: int = \"hello\" }").is_err());
}

// --- Edge cases ---

#[test]
fn test_check_edge_shadow() {
    // Shadow is allowed in inner scopes
    assert!(check("func f() { let x = 1 if true { let x = \"a\" print(x) } }").is_ok());
}

#[test]
fn test_check_edge_nested_scope() {
    assert!(check("func f() { let x = 1 { let x = 2 { let x = 3 } } }").is_ok());
}

#[test]
fn test_check_edge_empty_func() {
    assert!(check("func f() {}").is_ok());
}

#[test]
fn test_check_edge_return_in_if() {
    assert!(check("func f() -> int { if true { return 1 } else { return 2 } }").is_ok());
}

#[test]
fn test_check_edge_while_loop() {
    assert!(check("func f() { let i = 0 while i < 10 { i = i + 1 } }").is_ok());
}

#[test]
fn test_check_edge_for_loop() {
    assert!(check("func f() { for i in 0..10 { print(i) } }").is_ok());
}
