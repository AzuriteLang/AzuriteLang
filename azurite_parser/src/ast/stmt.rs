use crate::ast::{ClassField, EnumVariant, Expr, Ident, Type};

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
        params: Vec<crate::ast::Param>,
        return_type: Option<Type>,
        body: Box<Expr>,
    },
    Class {
        name: Ident,
        type_params: Vec<String>,
        parent: Option<Box<Type>>,
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
    For {
        name: Ident,
        iterable: Box<Expr>,
        body: Box<Expr>,
    },
    Return {
        value: Option<Box<Expr>>,
    },
    Break,
    Continue,
    Import {
        path: String,
        span: azurite_lexer::Span,
    },
    Expr(Expr),
}
