use azurite_parser::ast::*;
use crate::checker::Checker;
use crate::types::Type;
use azurite_lexer::Span;

fn resolve_instance_field(c: &mut Checker, instance: &Type, field: &str, span: Span) -> Option<Type> {
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

fn resolve_instance_method(c: &mut Checker, instance: &Type, method: &str, args: &[Expr], span: Span) -> Option<Type> {
    match instance {
        Type::Instance { name } => {
            let fn_name = format!("{}_{}", name, method);
            let sym_info = c.scope.lookup(&fn_name).map(|s| s.type_.clone());
            match sym_info {
                Some(Type::Func { params, ret }) => {
                    if !params.is_empty() && params.len() > args.len() {
                        c.error(span, format!("expected at least {} args, got {}", params.len(), args.len()));
                    }
                    for (i, arg) in args.iter().enumerate() {
                        let arg_type = super::expr::check_expr(c, arg);
                        if let (Some(expected), Some(actual)) = (params.get(i), arg_type) {
                            if expected != &actual {
                                c.error(arg.span(), format!("arg {}: expected '{}', got '{}'", i + 1, expected, actual));
                            }
                        }
                    }
                    Some(*ret.clone())
                }
                Some(_) => {
                    c.error(span, "not a function".to_string());
                    None
                }
                None => {
                    c.error(span, format!("no method '{}' on '{}'", method, name));
                    None
                }
            }
        }
        _ => {
            c.error(span, "cannot call method on non-instance".to_string());
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
        Expr::FieldAccess { obj, field, null_safe } => {
            let span = expr.span();
            let obj_type = check_expr(c, obj);
            if *null_safe && matches!(obj_type, Some(Type::Null)) {
                return Some(Type::Null);
            }
            // Check for enum variant access: Color.Red
            if let Some(Type::Void) = obj_type {
                if let Expr::Ident(ident) = obj.as_ref() {
                    if c.enums.contains_key(&ident.name) {
                        let is_variant = c.enums.get(&ident.name).map_or(false, |v| v.iter().any(|ev| ev.name.name == *field));
                        if is_variant {
                            return Some(Type::Instance { name: ident.name.clone() });
                        }
                    }
                }
            }
            match obj_type {
                Some(ref t) => resolve_instance_field(c, t, field, span),
                None => None,
            }
        }
        Expr::MethodCall { obj, method, args, null_safe } => {
            let span = obj.span();
            if method == "new" {
                if let Expr::Ident(ident) = obj.as_ref() {
                    if c.generic_classes.contains_key(&ident.name) {
                        return c.instantiate_generic_constructor(&ident.name, args);
                    }
                    if c.concrete_classes.contains_key(&ident.name) {
                        let fn_name = format!("{}_{}", ident.name, method);
                        let fn_type = c.scope.lookup(&fn_name).map(|s| s.type_.clone());
                        if let Some(Type::Func { params, ret }) = fn_type {
                            for (i, arg) in args.iter().enumerate() {
                                let arg_type = super::expr::check_expr(c, arg);
                                if let (Some(expected), Some(actual)) = (params.get(i), arg_type) {
                                    if *expected != actual {
                                        c.error(arg.span(), format!("arg {}: expected '{}', got '{}'", i + 1, expected, actual));
                                    }
                                }
                            }
                            return Some(*ret);
                        }
                    }
                }
            }
            if *null_safe && matches!(check_expr(c, obj), Some(Type::Null)) {
                return Some(Type::Null);
            }
            let obj_type = check_expr(c, obj);
            // Check for enum variant access in expression: Color.Red
            if let Some(Type::Void) = obj_type {
                if let Expr::Ident(ident) = obj.as_ref() {
                    if c.enums.contains_key(&ident.name) {
                        let is_variant = c.enums.get(&ident.name).map_or(false, |v| v.iter().any(|ev| ev.name.name == *method));
                        if is_variant {
                            for a in args { check_expr(c, a); }
                            return Some(Type::Instance { name: ident.name.clone() });
                        }
                    }
                }
            }
            match obj_type {
                Some(ref t) => resolve_instance_method(c, t, method, args, span),
                None => { for a in args { check_expr(c, a); } None }
            }
        }
        Expr::EnumVariant { enum_name, args, .. } => {
            for a in args { check_expr(c, a); }
            if c.enums.contains_key(enum_name) {
                Some(Type::Instance { name: enum_name.clone() })
            } else {
                Some(Type::Void)
            }
        }
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
            let span = obj.span();
            match check_expr(c, obj) {
                Some(Type::Array(elem)) => Some(*elem),
                Some(other) => { c.error(span, format!("cannot index '{}'", other)); None }
                None => None,
            }
        }
        Expr::Match { value, arms } => {
            let val_type = check_expr(c, value);
            // Check match exhaustiveness for enum types
            if let Some(Type::Instance { name: enum_name }) = val_type {
                if let Some(variants) = c.enums.get(&enum_name) {
                    let has_wildcard = arms.iter().any(|a| matches!(a.pattern, Pattern::Wildcard | Pattern::Ident(_)));
                    if !has_wildcard {
                        let covered: Vec<&str> = arms.iter().filter_map(|a| {
                            if let Pattern::EnumVariant { ref variant, .. } = a.pattern {
                                Some(variant.as_str())
                            } else { None }
                        }).collect();
                        let missing: Vec<&str> = variants.iter()
                            .filter(|v| !covered.contains(&v.name.name.as_str()))
                            .map(|v| v.name.name.as_str())
                            .collect();
                        if !missing.is_empty() {
                            c.error(expr.span(), format!("non-exhaustive match: missing variants {:?}", missing));
                        }
                    }
                }
            }
            for arm in arms { check_expr(c, &arm.body); }
            Some(Type::Void)
        }
        Expr::Range { start, end } => { check_expr(c, start); check_expr(c, end); Some(Type::Void) }
        Expr::Ident(ident) => {
            match c.scope.lookup(&ident.name) {
                Some(sym) => Some(sym.type_.clone()),
                None => {
                    if c.generic_classes.contains_key(&ident.name) || c.concrete_classes.contains_key(&ident.name) || c.enums.contains_key(&ident.name) {
                        Some(Type::Void)
                    } else {
                        c.error(ident.span, format!("undefined '{}'", ident.name));
                        None
                    }
                }
            }
        }
        Expr::Binary { left, right, op } => {
            let span = left.span();
            let l = check_expr(c, left);
            let r = check_expr(c, right);
            check_binary_op(c, l, r, *op, span)
        }
        Expr::Unary { op, operand } => {
            let span = operand.span();
            let t = check_expr(c, operand);
            check_unary_op(c, t, *op, span)
        }
        Expr::Call { callee, args } => {
            let span = callee.span();
            let callee_type = check_expr(c, callee);
            match callee_type {
                Some(Type::Func { params, ret }) => {
                    if !params.is_empty() && params.len() > args.len() {
                        c.error(span, format!("expected at least {} args, got {}", params.len(), args.len()));
                    }
                    for (i, arg) in args.iter().enumerate() {
                        let arg_type = check_expr(c, arg);
                        if let (Some(expected), Some(actual)) = (params.get(i), arg_type) {
                            if expected != &actual {
                                c.error(arg.span(), format!("arg {}: expected '{}', got '{}'", i + 1, expected, actual));
                            }
                        }
                    }
                    Some(*ret)
                }
                Some(other) => { c.error(span, format!("cannot call '{}'", other)); None }
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
        Expr::Tuple(elems) => {
            let types: Vec<Type> = elems.iter().filter_map(|e| check_expr(c, e)).collect();
            if types.len() == elems.len() { Some(Type::Tuple(types)) } else { None }
        }
    }
}

fn check_binary_op(c: &mut Checker, l: Option<Type>, r: Option<Type>, op: BinOp, span: Span) -> Option<Type> {
    let lt = l.unwrap_or(Type::Void);
    let rt = r.unwrap_or(Type::Void);
    match op {
        BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
            if lt.is_numeric() && rt.is_numeric() {
                Some(if lt == Type::Float || rt == Type::Float { Type::Float } else { Type::Int })
            } else if op == BinOp::Add && lt == Type::String && rt == Type::String {
                Some(Type::String)
            } else { c.error(span, format!("cannot apply '{}' to '{}' and '{}'", op, lt, rt)); None }
        }
        BinOp::Eq | BinOp::Neq => {
            if lt == rt || (lt.is_numeric() && rt.is_numeric()) { Some(Type::Bool) }
            else { c.error(span, format!("cannot compare '{}' with '{}'", lt, rt)); None }
        }
        BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => {
            if lt.is_numeric() && rt.is_numeric() { Some(Type::Bool) }
            else { c.error(span, format!("cannot compare '{}' with '{}'", lt, rt)); None }
        }
        BinOp::And | BinOp::Or => {
            if lt == Type::Bool && rt == Type::Bool { Some(Type::Bool) }
            else { c.error(span, format!("cannot apply '{}' to '{}' and '{}'", op, lt, rt)); None }
        }
        BinOp::Assign => {
            if lt == rt || lt == Type::Null { Some(rt) }
            else { c.error(span, format!("cannot assign '{}' to '{}'", rt, lt)); None }
        }
        BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::Shl | BinOp::Shr => {
            if lt == Type::Int && rt == Type::Int { Some(Type::Int) }
            else { c.error(span, format!("bitwise op requires ints")); None }
        }
    }
}

fn check_unary_op(c: &mut Checker, t: Option<Type>, op: UnOp, span: Span) -> Option<Type> {
    let t = t.unwrap_or(Type::Void);
    match op {
        UnOp::Neg if t.is_numeric() => Some(t),
        UnOp::Neg => { c.error(span, format!("cannot negate '{}'", t)); None }
        UnOp::Not if t == Type::Bool => Some(Type::Bool),
        UnOp::Not => { c.error(span, format!("cannot apply 'not' to '{}'", t)); None }
    }
}
