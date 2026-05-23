use azurite_parser::ast::*;
use azurite_test::{check, parse_prog};

#[test]
fn test_break_for_loop() {
    assert!(check("func main() { for i in 0..5 { if i == 3 { break } } }").is_ok());
}

#[test]
fn test_continue_for_loop() {
    assert!(check("func main() { for i in 0..5 { if i == 2 { continue } } }").is_ok());
}

#[test]
fn test_break_while_loop() {
    assert!(check("func main() { let i = 0 while i < 5 { if i == 3 { break } i = i + 1 } }").is_ok());
}

#[test]
fn test_continue_while_loop() {
    assert!(check("func main() { let i = 0 while i < 5 { i = i + 1 if i == 2 { continue } } }").is_ok());
}

#[test]
fn test_break_outside_loop_fails() {
    assert!(check("func main() { break }").is_err());
}

#[test]
fn test_continue_outside_loop_fails() {
    assert!(check("func main() { continue }").is_err());
}

#[test]
fn test_array_literal_type() {
    assert!(check("func main() { let a = [1, 2, 3] }").is_ok());
}

#[test]
fn test_array_index() {
    assert!(check("func main() { let a = [1, 2, 3] print(a[0]) }").is_ok());
}

#[test]
fn test_array_index_float() {
    assert!(check("func main() { let a = [1.5, 2.5] print(a[0]) }").is_ok());
}

#[test]
fn test_else_if_sugar() {
    let prog = parse_prog("func main() { if true { } else if false { } else { } }");
    assert_eq!(prog.statements.len(), 1);
    if let Stmt::Func { body, .. } = &prog.statements[0] {
        if let Expr::Block(stmts) = body.as_ref() {
            if let Some(Stmt::If { else_branch, .. }) = stmts.first() {
                assert!(else_branch.is_some());
            } else { panic!("expected if stmt in block"); }
        } else { panic!("expected block body"); }
    }
}

#[test]
fn test_generic_return_type() {
    // Method returning T from a generic class
    assert!(check("class Box<T> { value: T func get(self) -> T { return self.value } } func main() { let b = Box.new(42) let v = b.get() print(v) }").is_ok());
}
