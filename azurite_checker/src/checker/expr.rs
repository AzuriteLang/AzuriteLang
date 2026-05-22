use azurite_parser::ast::*;
use crate::checker::Checker;
use crate::types::Type;

pub fn check_expr(c: &mut Checker, expr: &Expr) -> Option<Type> {
    match expr {
        Expr::Int(_) => Some(Type::Int),
        Expr::Float(_) => Some(Type::Float),
        Expr::String(_) => Some(Type::String),
        Expr::Char(_) => Some(Type::Int),
        Expr::Bool(_) => Some(Type::Bool),
        Expr::Null => Some(Type::Null),
        Expr::Self_ | Expr::Super => Some(Type::Void),
        Expr::FieldAccess { obj, .. } => check_expr(c, obj),
        Expr::MethodCall { obj, args, .. } => { check_expr(c, obj); for a in args { check_expr(c, a); } Some(Type::Void) }
        Expr::EnumVariant { args, .. } => { for a in args { check_expr(c, a); } Some(Type::Void) }
        Expr::Array(elems) => { for e in elems { check_expr(c, e); } Some(Type::Void) }
        Expr::Index { obj, index } => { check_expr(c, obj); check_expr(c, index); Some(Type::Void) }
        Expr::Match { value, arms } => { check_expr(c, value); for arm in arms { check_expr(c, &arm.body); } Some(Type::Void) }
        Expr::Range { start, end } => { check_expr(c, start); check_expr(c, end); Some(Type::Void) }
        Expr::Ident(ident) => {
            match c.scope.lookup(&ident.name) {
                Some(sym) => Some(sym.type_.clone()),
                None => { c.error(ident.span, format!("undefined '{}'", ident.name)); None }
            }
        }
        Expr::Binary { left, op, right } => {
            let l = check_expr(c, left);
            let r = check_expr(c, right);
            check_binary_op(c, l, r, *op)
        }
        Expr::Unary { op, operand } => {
            let t = check_expr(c, operand);
            check_unary_op(c, t, *op)
        }
        Expr::Call { callee, args } => {
            let callee_type = check_expr(c, callee);
            match callee_type {
                Some(Type::Func { params, ret }) => {
                    if !params.is_empty() && params.len() != args.len() {
                        c.error(azurite_lexer::Span::new(0, 0, 0, 0), format!("expected {} args, got {}", params.len(), args.len()));
                    }
                    for (i, arg) in args.iter().enumerate() {
                        let arg_type = check_expr(c, arg);
                        if let (Some(expected), Some(actual)) = (params.get(i), arg_type) {
                            if expected != &actual {
                                c.error(azurite_lexer::Span::new(0, 0, 0, 0), format!("arg {}: expected '{}', got '{}'", i + 1, expected, actual));
                            }
                        }
                    }
                    Some(*ret)
                }
                Some(other) => { c.error(azurite_lexer::Span::new(0, 0, 0, 0), format!("cannot call '{}'", other)); None }
                None => None,
            }
        }
        Expr::Block(stmts) => {
            c.scope.push();
            let mut last = None;
            for stmt in stmts { last = super::stmt::check_stmt(c, stmt).or(last); }
            c.scope.pop();
            last
        }
        Expr::If { condition, then_branch, else_branch } => {
            check_expr(c, condition);
            let t = check_expr(c, then_branch);
            let e = else_branch.as_ref().map(|b| check_expr(c, b)).flatten();
            t.or(e)
        }
        Expr::While { condition, body } => {
            check_expr(c, condition);
            check_expr(c, body);
            Some(Type::Void)
        }
    }
}

fn check_binary_op(c: &mut Checker, l: Option<Type>, r: Option<Type>, op: BinOp) -> Option<Type> {
    let lt = l.unwrap_or(Type::Void);
    let rt = r.unwrap_or(Type::Void);
    match op {
        BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
            if lt.is_numeric() && rt.is_numeric() {
                Some(if lt == Type::Float || rt == Type::Float { Type::Float } else { Type::Int })
            } else if op == BinOp::Add && lt == Type::String && rt == Type::String {
                Some(Type::String)
            } else { c.error(azurite_lexer::Span::new(0, 0, 0, 0), format!("cannot apply '{}' to '{}' and '{}'", op, lt, rt)); None }
        }
        BinOp::Eq | BinOp::Neq => {
            if lt == rt || (lt.is_numeric() && rt.is_numeric()) { Some(Type::Bool) }
            else { c.error(azurite_lexer::Span::new(0, 0, 0, 0), format!("cannot compare '{}' with '{}'", lt, rt)); None }
        }
        BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => {
            if lt.is_numeric() && rt.is_numeric() { Some(Type::Bool) }
            else { c.error(azurite_lexer::Span::new(0, 0, 0, 0), format!("cannot compare '{}' with '{}'", lt, rt)); None }
        }
        BinOp::And | BinOp::Or => {
            if lt == Type::Bool && rt == Type::Bool { Some(Type::Bool) }
            else { c.error(azurite_lexer::Span::new(0, 0, 0, 0), format!("cannot apply '{}' to '{}' and '{}'", op, lt, rt)); None }
        }
        BinOp::Assign => {
            if lt == rt || lt == Type::Null { Some(rt) }
            else { c.error(azurite_lexer::Span::new(0, 0, 0, 0), format!("cannot assign '{}' to '{}'", rt, lt)); None }
        }
        BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::Shl | BinOp::Shr => {
            if lt == Type::Int && rt == Type::Int { Some(Type::Int) }
            else { c.error(azurite_lexer::Span::new(0, 0, 0, 0), format!("bitwise op requires ints")); None }
        }
    }
}

fn check_unary_op(c: &mut Checker, t: Option<Type>, op: UnOp) -> Option<Type> {
    let t = t.unwrap_or(Type::Void);
    match op {
        UnOp::Neg if t.is_numeric() => Some(t),
        UnOp::Neg => { c.error(azurite_lexer::Span::new(0, 0, 0, 0), format!("cannot negate '{}'", t)); None }
        UnOp::Not if t == Type::Bool => Some(Type::Bool),
        UnOp::Not => { c.error(azurite_lexer::Span::new(0, 0, 0, 0), format!("cannot apply 'not' to '{}'", t)); None }
    }
}
