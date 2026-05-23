use crate::ast::{BinOp, Ident, Stmt, UnOp};

#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    Wildcard,
    Int(i64),
    Bool(bool),
    String(String),
    Ident(String),
    EnumVariant {
        enum_name: Option<String>,
        variant: String,
        bindings: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Box<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Int(i64),
    Float(f64),
    String(String),
    Char(char),
    Bool(bool),
    Null,
    Ident(Ident),
    Self_,
    Super,
    Binary {
        left: Box<Expr>,
        op: BinOp,
        right: Box<Expr>,
    },
    Unary {
        op: UnOp,
        operand: Box<Expr>,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
    MethodCall {
        obj: Box<Expr>,
        method: String,
        args: Vec<Expr>,
        null_safe: bool,
    },
    FieldAccess {
        obj: Box<Expr>,
        field: String,
        null_safe: bool,
    },
    EnumVariant {
        enum_name: String,
        variant: String,
        args: Vec<Expr>,
    },
    Array(Vec<Expr>),
    Index {
        obj: Box<Expr>,
        index: Box<Expr>,
    },
    Slice {
        obj: Box<Expr>,
        start: Box<Expr>,
        end: Box<Expr>,
        end_is_len: bool, // true when end is omitted (to end of string)
    },
    Match {
        value: Box<Expr>,
        arms: Vec<MatchArm>,
    },
    Range {
        start: Box<Expr>,
        end: Box<Expr>,
    },
    Block(Vec<Stmt>),
    If {
        condition: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Option<Box<Expr>>,
    },
    While {
        condition: Box<Expr>,
        body: Box<Expr>,
    },
    Tuple(Vec<Expr>),
}

impl Expr {
    pub fn span(&self) -> azurite_lexer::Span {
        match self {
            Expr::Ident(i) => i.span,
            Expr::Binary { left, .. } => left.span(),
            Expr::Unary { operand, .. } => operand.span(),
            Expr::Call { callee, .. } => callee.span(),
            Expr::MethodCall { obj, .. } => obj.span(),
            Expr::FieldAccess { obj, .. } => obj.span(),
            Expr::Index { obj, .. } => obj.span(),
            Expr::Slice { obj, .. } => obj.span(),
            Expr::Block(stmts) => stmts.first().map(|s| s.span()).unwrap_or(azurite_lexer::Span::new(0, 0, 0, 0)),
            Expr::If { condition, .. } => condition.span(),
            Expr::While { condition, .. } => condition.span(),
            Expr::Match { value, .. } => value.span(),
            Expr::Range { start, .. } => start.span(),
            Expr::EnumVariant { args, .. } => args.first().map(|a| a.span()).unwrap_or(azurite_lexer::Span::new(0, 0, 0, 0)),
            Expr::Array(items) => items.first().map(|a| a.span()).unwrap_or(azurite_lexer::Span::new(0, 0, 0, 0)),
            Expr::Tuple(items) => items.first().map(|a| a.span()).unwrap_or(azurite_lexer::Span::new(0, 0, 0, 0)),
            _ => azurite_lexer::Span::new(0, 0, 0, 0),
        }
    }
}
