use azurite_lexer::Lexer;
use azurite_parser::Parser;
use azurite_checker::Checker;
use azurite_errors::AzError;

fn check(src: &str) -> Result<(), Vec<AzError>> {
    let tokens = Lexer::new(src).tokenize().unwrap();
    let prog = Parser::new(tokens).parse_program().unwrap();
    Checker::new().check_program(&prog)
}

#[test]
fn test_accepts_valid_program() {
    assert!(check("func main() { let x: int = 42 let y = x + 1 print(0) }").is_ok());
}

#[test]
fn test_accepts_return_match() {
    assert!(check("func add(a: int, b: int) -> int { return a + b }").is_ok());
}

#[test]
fn test_accepts_numeric_coercion() {
    assert!(check("func main() { let a: int = 42 let b: float = 3.14 let c = a + b }").is_ok());
}

#[test]
fn test_accepts_void_return() {
    assert!(check("func main() { return }").is_ok());
}

#[test]
fn test_accepts_nested_scope() {
    assert!(check("func main() { let x = \"a\" if true { let x = \"b\" print(x) } }").is_ok());
}
