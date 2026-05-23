use crate::ast::{Expr, Ident};

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Name(String),
    Generic { name: String, params: Vec<Type> },
    Ptr(Box<Type>),
    Array(Box<Type>, Option<usize>),
    Tuple(Vec<Type>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: Ident,
    pub type_annotation: Option<Type>,
    pub vararg: bool,
    pub default_value: Option<Box<Expr>>,
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
