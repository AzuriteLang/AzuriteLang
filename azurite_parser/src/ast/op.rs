use std::fmt;

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
            BinOp::Add => write!(f, "+"), BinOp::Sub => write!(f, "-"),
            BinOp::Mul => write!(f, "*"), BinOp::Div => write!(f, "/"),
            BinOp::Mod => write!(f, "%"), BinOp::Eq => write!(f, "=="),
            BinOp::Neq => write!(f, "!="), BinOp::Lt => write!(f, "<"),
            BinOp::Gt => write!(f, ">"), BinOp::Le => write!(f, "<="),
            BinOp::Ge => write!(f, ">="), BinOp::And => write!(f, "and"),
            BinOp::Or => write!(f, "or"), BinOp::BitAnd => write!(f, "&"),
            BinOp::BitOr => write!(f, "|"), BinOp::BitXor => write!(f, "^"),
            BinOp::Shl => write!(f, "<<"), BinOp::Shr => write!(f, ">>"),
            BinOp::Assign => write!(f, "="),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnOp {
    Neg,
    Not,
}
