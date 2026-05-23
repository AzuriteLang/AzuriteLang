use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Int,
    Float,
    String,
    Bool,
    Null,
    Void,
    Instance { name: String },
    Array(Box<Type>),
    Func {
        params: Vec<Type>,
        ret: Box<Type>,
    },
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Int => write!(f, "int"),
            Type::Float => write!(f, "float"),
            Type::String => write!(f, "string"),
            Type::Bool => write!(f, "bool"),
            Type::Null => write!(f, "null"),
            Type::Void => write!(f, "void"),
            Type::Instance { name } => write!(f, "{}", name),
            Type::Array(elem) => write!(f, "{}[]", elem),
            Type::Func { params, ret } => {
                let params: Vec<String> = params.iter().map(|p| p.to_string()).collect();
                write!(f, "func({}) -> {}", params.join(", "), ret)
            }
        }
    }
}

impl Type {
    pub fn from_name(name: &str) -> Option<Type> {
        match name {
            "int" => Some(Type::Int),
            "float" => Some(Type::Float),
            "string" => Some(Type::String),
            "bool" => Some(Type::Bool),
            "null" => Some(Type::Null),
            "void" => Some(Type::Void),
            _ => None,
        }
    }

    pub fn is_numeric(&self) -> bool {
        matches!(self, Type::Int | Type::Float)
    }
}
