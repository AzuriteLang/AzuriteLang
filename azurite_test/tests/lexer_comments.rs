use azurite_lexer::{Lexer, TokenKind};

#[test]
fn test_comments() {
    let tokens = Lexer::new("42 // this is a comment\n 10 /* block */ 20").tokenize().unwrap();
    assert_eq!(tokens[0].kind, TokenKind::Int(42));
    assert_eq!(tokens[1].kind, TokenKind::Int(10));
    assert_eq!(tokens[2].kind, TokenKind::Int(20));
}

#[test]
fn test_comment_line_only() {
    let tokens = Lexer::new("// just a comment\n").tokenize().unwrap();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].kind, TokenKind::EOF);
}

#[test]
fn test_comment_block_multiline() {
    let tokens = Lexer::new("42 /* line1\nline2\nline3 */ 99").tokenize().unwrap();
    assert_eq!(tokens[0].kind, TokenKind::Int(42));
    assert_eq!(tokens[1].kind, TokenKind::Int(99));
}

#[test]
fn test_comment_block_empty() {
    let tokens = Lexer::new("/**/ 42").tokenize().unwrap();
    assert_eq!(tokens[0].kind, TokenKind::Int(42));
}

#[test]
fn test_comment_unterminated_block() {
    let tokens = Lexer::new("42 /* oops").tokenize().unwrap();
    assert_eq!(tokens[0].kind, TokenKind::Int(42));
    assert_eq!(tokens[1].kind, TokenKind::EOF);
}

#[test]
fn test_comment_after_code() {
    let tokens = Lexer::new("42 // end of line").tokenize().unwrap();
    assert_eq!(tokens[0].kind, TokenKind::Int(42));
    assert_eq!(tokens[1].kind, TokenKind::EOF);
}

#[test]
fn test_large_program() {
    let src = r#"
        func main() {
            let name: string = "Azurite"
            let version = 42
            if version > 0 { print(name) }
        }
    "#;
    let tokens = Lexer::new(src).tokenize().unwrap();
    assert_eq!(tokens[0].kind, TokenKind::Func);
    assert_eq!(tokens[5].kind, TokenKind::Let);
    assert_eq!(tokens[11].kind, TokenKind::Let);
    assert_eq!(tokens[15].kind, TokenKind::If);
}
