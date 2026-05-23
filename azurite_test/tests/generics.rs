use azurite_checker::Checker;
use azurite_lexer::Lexer;
use azurite_parser::Parser;

fn check(src: &str) -> Result<(), Vec<azurite_errors::AzError>> {
    let tokens = Lexer::new(src).tokenize().unwrap();
    let prog = Parser::new(tokens).parse_program().unwrap();
    Checker::new().check_program(&prog)
}

#[test]
fn test_generic_box_new_int() {
    assert!(check("class Box<T> { value: T } func main() { let b = Box.new(42) }").is_ok());
}

#[test]
fn test_generic_box_new_float() {
    assert!(check("class Box<T> { value: T } func main() { let b = Box.new(3.14) }").is_ok());
}

#[test]
fn test_generic_box_new_string() {
    assert!(check("class Box<T> { value: T } func main() { let b = Box.new(\"hello\") }").is_ok());
}

#[test]
fn test_generic_box_new_bool() {
    assert!(check("class Box<T> { value: T } func main() { let b = Box.new(true) }").is_ok());
}

#[test]
fn test_generic_box_field_access_int() {
    assert!(check("class Box<T> { value: T } func main() { let b = Box.new(42); print(b.value) }").is_ok());
}

#[test]
fn test_generic_box_field_access_float() {
    assert!(check("class Box<T> { value: T } func main() { let b = Box.new(3.14); print(b.value) }").is_ok());
}

#[test]
fn test_generic_box_explicit_method() {
    assert!(check("class Box<T> { value: T func new(v: T) { self.value = v } func get(self) -> T { return self.value } } func main() { let b = Box.new(42); let v = b.get(); print(v) }").is_ok());
}

#[test]
fn test_generic_box_multiple_instances() {
    assert!(check("class Box<T> { value: T } func main() { let a = Box.new(1); let b = Box.new(2); let c = Box.new(3) }").is_ok());
}

#[test]
fn test_generic_box_with_expression_arg() {
    assert!(check("class Box<T> { value: T } func double(x: int) -> int { return x * 2 } func main() { let b = Box.new(double(21)) }").is_ok());
} // end of tests module
