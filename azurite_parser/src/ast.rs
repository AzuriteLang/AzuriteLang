use std::fmt;
use azurite_lexer::Span;

pub type NodeId = usize;

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub statements: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Let {
        name: Ident,
        type_annotation: Option<Type>,
        value: Box<Expr>,
    },
    Func {
        name: Ident,
        params: Vec<Param>,
        return_type: Option<Type>,
        body: Box<Expr>,
    },
    Class {
        name: Ident,
        fields: Vec<ClassField>,
        methods: Vec<Stmt>,
    },
    Enum {
        name: Ident,
        variants: Vec<EnumVariant>,
    },
    If {
        condition: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Option<Box<Expr>>,
    },
    While {
        condition: Box<Expr>,
        body: Box<Expr>,
    },
    Return {
        value: Option<Box<Expr>>,
    },
    For {
        name: Ident,
        iterable: Box<Expr>,
        body: Box<Expr>,
    },
    Import { path: String, span: Span },
    Expr(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClassField {
    pub name: Ident,
    pub type_: Type,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumVariant {
    pub name: Ident,
    pub types: Vec<Type>,
}

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
    },
    FieldAccess {
        obj: Box<Expr>,
        field: String,
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
}

#[derive(Debug, Clone, PartialEq)]
pub struct Ident {
    pub name: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: Ident,
    pub type_annotation: Option<Type>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Name(String),
    Ptr(Box<Type>),
    Array(Box<Type>, Option<usize>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinOp {
    Add, Sub, Mul, Div, Mod,
    Eq, Neq, Lt, Gt, Le, Ge,
    And, Or,
    BitAnd, BitOr, BitXor, Shl, Shr,
    Assign,
}

impl fmt::Display for BinOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BinOp::Add => write!(f, "+"),
            BinOp::Sub => write!(f, "-"),
            BinOp::Mul => write!(f, "*"),
            BinOp::Div => write!(f, "/"),
            BinOp::Mod => write!(f, "%"),
            BinOp::Eq => write!(f, "=="),
            BinOp::Neq => write!(f, "!="),
            BinOp::Lt => write!(f, "<"),
            BinOp::Gt => write!(f, ">"),
            BinOp::Le => write!(f, "<="),
            BinOp::Ge => write!(f, ">="),
            BinOp::And => write!(f, "and"),
            BinOp::Or => write!(f, "or"),
            BinOp::BitAnd => write!(f, "&"),
            BinOp::BitOr => write!(f, "|"),
            BinOp::BitXor => write!(f, "^"),
            BinOp::Shl => write!(f, "<<"),
            BinOp::Shr => write!(f, ">>"),
            BinOp::Assign => write!(f, "="),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnOp {
    Neg,
    Not,
}
