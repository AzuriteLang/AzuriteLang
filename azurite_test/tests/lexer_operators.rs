use azurite_lexer::{Lexer, TokenKind};

#[test]
fn test_operators() {
    let src = "+ - * / % = == != < > <= >= && || ! & | ^ << >>";
    let tokens = Lexer::new(src).tokenize().unwrap();
    let expected = [
        TokenKind::Plus, TokenKind::Minus, TokenKind::Star, TokenKind::Slash,
        TokenKind::Percent, TokenKind::Assign, TokenKind::Equal, TokenKind::NotEqual,
        TokenKind::Less, TokenKind::Greater, TokenKind::LessEqual, TokenKind::GreaterEqual,
        TokenKind::AndAnd, TokenKind::OrOr, TokenKind::Not, TokenKind::BitAnd,
        TokenKind::BitOr, TokenKind::BitXor, TokenKind::Shl, TokenKind::Shr,
    ];
    for (i, exp) in expected.iter().enumerate() {
        assert_eq!(tokens[i].kind, *exp);
    }
}

#[test]
fn test_operators_no_space() {
    let tokens = Lexer::new("== != <= >= && || << >> ->").tokenize().unwrap();
    assert_eq!(tokens[0].kind, TokenKind::Equal);
    assert_eq!(tokens[1].kind, TokenKind::NotEqual);
    assert_eq!(tokens[2].kind, TokenKind::LessEqual);
    assert_eq!(tokens[3].kind, TokenKind::GreaterEqual);
    assert_eq!(tokens[4].kind, TokenKind::AndAnd);
    assert_eq!(tokens[5].kind, TokenKind::OrOr);
    assert_eq!(tokens[6].kind, TokenKind::Shl);
    assert_eq!(tokens[7].kind, TokenKind::Shr);
    assert_eq!(tokens[8].kind, TokenKind::Arrow);
}

#[test]
fn test_arrow_vs_minus() {
    let tokens = Lexer::new("-> - ->").tokenize().unwrap();
    assert_eq!(tokens[0].kind, TokenKind::Arrow);
    assert_eq!(tokens[1].kind, TokenKind::Minus);
    assert_eq!(tokens[2].kind, TokenKind::Arrow);
}

#[test]
fn test_delimiters() {
    let tokens = Lexer::new("() {} [] , ; : . -> #").tokenize().unwrap();
    assert_eq!(tokens[0].kind, TokenKind::LParen);
    assert_eq!(tokens[1].kind, TokenKind::RParen);
    assert_eq!(tokens[2].kind, TokenKind::LBrace);
    assert_eq!(tokens[3].kind, TokenKind::RBrace);
    assert_eq!(tokens[4].kind, TokenKind::LBracket);
    assert_eq!(tokens[5].kind, TokenKind::RBracket);
    assert_eq!(tokens[6].kind, TokenKind::Comma);
    assert_eq!(tokens[7].kind, TokenKind::Semicolon);
    assert_eq!(tokens[8].kind, TokenKind::Colon);
    assert_eq!(tokens[9].kind, TokenKind::Dot);
    assert_eq!(tokens[10].kind, TokenKind::Arrow);
    assert_eq!(tokens[11].kind, TokenKind::Hash);
}

#[test]
fn test_bit_ops() {
    let tokens = Lexer::new("a & b | c ^ d << e >> f").tokenize().unwrap();
    assert_eq!(tokens[1].kind, TokenKind::BitAnd);
    assert_eq!(tokens[3].kind, TokenKind::BitOr);
    assert_eq!(tokens[5].kind, TokenKind::BitXor);
    assert_eq!(tokens[7].kind, TokenKind::Shl);
    assert_eq!(tokens[9].kind, TokenKind::Shr);
}

#[test]
fn test_chained_comparison() {
    let tokens = Lexer::new("a <= b && c >= d").tokenize().unwrap();
    assert_eq!(tokens[1].kind, TokenKind::LessEqual);
    assert_eq!(tokens[3].kind, TokenKind::AndAnd);
    assert_eq!(tokens[5].kind, TokenKind::GreaterEqual);
}
