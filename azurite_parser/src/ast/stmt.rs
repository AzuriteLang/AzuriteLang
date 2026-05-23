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
    Destructure {
        names: Vec<Ident>,
        value: Box<Expr>,
    },
    Try {
        try_block: Box<Expr>,
        catch_var: Ident,
        catch_block: Box<Expr>,
    },
    Throw {
        value: Box<Expr>,
    },
}

impl Stmt {
    pub fn span(&self) -> azurite_lexer::Span {
        match self {
            Stmt::Let { name, .. } => name.span,
            Stmt::Func { name, .. } => name.span,
            Stmt::Class { name, .. } => name.span,
            Stmt::Enum { name, .. } => name.span,
            Stmt::Return { value } => value.as_ref().map(|v| v.span()).unwrap_or(azurite_lexer::Span::new(0, 0, 0, 0)),
            Stmt::Break => azurite_lexer::Span::new(0, 0, 0, 0),
            Stmt::Continue => azurite_lexer::Span::new(0, 0, 0, 0),
            Stmt::Import { span, .. } => *span,
            Stmt::If { condition, .. } => condition.span(),
            Stmt::While { condition, .. } => condition.span(),
            Stmt::For { name, .. } => name.span,
            Stmt::Expr(e) => e.span(),
            Stmt::Destructure { value, .. } => value.span(),
            Stmt::Try { try_block, .. } => try_block.span(),
            Stmt::Throw { value, .. } => value.span(),
        }
    }
}
