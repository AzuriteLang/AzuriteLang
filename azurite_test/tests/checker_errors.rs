use azurite_test::check;

#[test]
fn test_rejects_type_mismatch() {
    assert!(check("func main() { let x: int = \"hello\" }").is_err());
}

#[test]
fn test_rejects_undefined_var() {
    assert!(check("func main() { print(x) }").is_err());
}

#[test]
fn test_rejects_bad_binary_op() {
    assert!(check("func main() { let x = true + 1 }").is_err());
}

#[test]
fn test_rejects_return_mismatch() {
    assert!(check("func main() -> int { return \"hello\" }").is_err());
}

#[test]
fn test_rejects_no_return_value() {
    assert!(check("func main() -> int { return }").is_err());
}

#[test]
fn test_rejects_double_decl() {
    assert!(check("func main() { let x = 1 let x = 2 }").is_err());
}
