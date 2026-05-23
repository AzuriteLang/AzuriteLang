use azurite_parser::ast::*;
use crate::checker::Checker;
use crate::types::Type;

fn resolve_instance_field(c: &mut Checker, instance: &Type, field: &str, span: azurite_lexer::Span) -> Option<Type> {
    match instance {
        Type::Instance { name } => {
            let field_type_opt = c.concrete_classes.get(name).and_then(|fields| {
                fields.iter().find(|f| f.name.name == field).map(|f| f.type_.clone())
            });
            match field_type_opt {
                Some(ast_type) => c.resolve_type(&ast_type),
                None => {
                    c.error(span, format!("no field '{}' on '{}'", field, name));
                    None
                }
            }
        }
        _ => {
            c.error(span, "cannot access field on non-instance".to_string());
            None
        }
    }
}

fn resolve_instance_method(c: &mut Checker, instance: &Type, method: &str, args: &[Expr], _span: azurite_lexer::Span) -> Option<Type> {
    match instance {
        Type::Instance { name } => {
            let fn_name = format!("{}_{}", name, method);
            let sym_info = c.scope.lookup(&fn_name).map(|s| s.type_.clone());
            match sym_info {
                Some(Type::Func { params, ret }) => {
                    if !params.is_empty() && params.len() != args.len() {
                        c.error(azurite_lexer::Span::new(0, 0, 0, 0), format!("expected {} args, got {}", params.len(), args.len()));
                    }
                    for (i, arg) in args.iter().enumerate() {
                        let arg_type = super::expr::check_expr(c, arg);
                        if let (Some(expected), Some(actual)) = (params.get(i), arg_type) {
                            if expected != &actual {
                                c.error(azurite_lexer::Span::new(0, 0, 0, 0), format!("arg {}: expected '{}', got '{}'", i + 1, expected, actual));
                            }
                        }
                    }
                    Some(*ret.clone())
                }
                Some(_) => {
                    c.error(azurite_lexer::Span::new(0, 0, 0, 0), "not a function".to_string());
                    None
                }
                None => {
                    c.error(azurite_lexer::Span::new(0, 0, 0, 0), format!("no method '{}' on '{}'", method, name));
                    None
                }
            }
        }
        _ => {
            c.error(azurite_lexer::Span::new(0, 0, 0, 0), "cannot call method on non-instance".to_string());
            None
        }
    }
}

pub fn check_expr(c: &mut Checker, expr: &Expr) -> Option<Type> {
    match expr {
        Expr::Int(_) => Some(Type::Int),
        Expr::Float(_) => Some(Type::Float),
        Expr::String(_) => Some(Type::String),
        Expr::Char(_) => Some(Type::Int),
        Expr::Bool(_) => Some(Type::Bool),
        Expr::Null => Some(Type::Null),
        Expr::Self_ | Expr::Super => Some(Type::Void),
        Expr::FieldAccess { obj, field } => {
            let obj_type = check_expr(c, obj);
            match obj_type {
                Some(ref t) => resolve_instance_field(c, t, field, azurite_lexer::Span::new(0, 0, 0, 0)),
                None => None,
            }
        }
        Expr::MethodCall { obj, method, args } => {
            if method == "new" {
                if let Expr::Ident(ident) = obj.as_ref() {
                    if c.generic_classes.contains_key(&ident.name) {
                        return c.instantiate_generic_constructor(&ident.name, args);
                    }
                }
            }
            let obj_type = check_expr(c, obj);
            match obj_type {
                Some(ref t) => resolve_instance_method(c, t, method, args, azurite_lexer::Span::new(0, 0, 0, 0)),
                None => { for a in args { check_expr(c, a); } None }
            }
        }
        Expr::EnumVariant { args, .. } => { for a in args { check_expr(c, a); } Some(Type::Void) }
        Expr::Array(elems) => {
            let mut elem_type = None;
            for e in elems {
                let t = check_expr(c, e);
                if elem_type.is_none() { elem_type = t; }
            }
            elem_type.map(|t| Type::Array(Box::new(t)))
        }
        Expr::Index { obj, index } => {
            check_expr(c, index);
            match check_expr(c, obj) {
                Some(Type::Array(elem)) => Some(*elem),
                Some(other) => { c.error(azurite_lexer::Span::new(0, 0, 0, 0), format!("cannot index '{}'", other)); None }
                None => None,
            }
        }
        Expr::Match { value, arms } => { check_expr(c, value); for arm in arms { check_expr(c, &arm.body); } Some(Type::Void) }
        Expr::Range { start, end } => { check_expr(c, start); check_expr(c, end); Some(Type::Void) }
        Expr::Ident(ident) => {
            match c.scope.lookup(&ident.name) {
                Some(sym) => Some(sym.type_.clone()),
                None => {
                    if c.generic_classes.contains_key(&ident.name) || c.concrete_classes.contains_key(&ident.name) {
                        Some(Type::Void)
                    } else {
                        c.error(ident.span, format!("undefined '{}'", ident.name));
                        None
                    }
                }
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
            c.in_loop += 1;
            check_expr(c, condition);
            check_expr(c, body);
            c.in_loop -= 1;
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
