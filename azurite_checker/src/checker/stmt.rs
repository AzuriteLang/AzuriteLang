use azurite_parser::ast::*;
use crate::checker::Checker;
use crate::symbol::{Symbol, SymbolKind};
use crate::types::Type;

pub fn check_stmt(c: &mut Checker, stmt: &Stmt) -> Option<Type> {
    match stmt {
        Stmt::Let { name, type_annotation, value } => {
            let inferred = super::expr::check_expr(c, value);
            let declared = type_annotation.as_ref().and_then(|t| c.resolve_type(t));
            let type_ = match (inferred, declared) {
                (Some(inf), Some(dec)) => {
                    if inf != dec { c.error(name.span, format!("type mismatch: expected '{}', got '{}'", dec, inf)); }
                    Some(dec)
                }
                (Some(inf), None) => {
                    if inf == Type::Null { c.error(name.span, format!("cannot infer type for 'let {}'", name.name)); }
                    Some(inf)
                }
                (None, Some(dec)) => Some(dec),
                (None, None) => None,
            };
            if let Some(ref t) = type_ {
                c.scope.insert(&name.name, Symbol { name: name.name.clone(), kind: SymbolKind::Variable, type_: t.clone() })
                    .unwrap_or_else(|e| c.error(name.span, e));
            }
            type_
        }
        Stmt::Import { .. } | Stmt::Enum { .. } => None,
        Stmt::Class { name, type_params, parent: _, fields, methods } => {
            if !type_params.is_empty() {
                c.generic_classes.insert(name.name.clone(), (type_params.clone(), fields.clone(), methods.clone()));
                return None;
            }
            let concrete_name = name.name.clone();
            c.concrete_classes.insert(concrete_name.clone(), fields.clone());
            for method in methods {
                if let Stmt::Func { name: mname, params: mparams, return_type, .. } = method {
                    let fn_name = format!("{}_{}", concrete_name, mname.name);
                    let resolved_params: Vec<Type> = mparams.iter().filter(|p| p.name.name != "self" && !p.vararg).map(|p| {
                        p.type_annotation.as_ref().and_then(|t| c.resolve_type(t)).unwrap_or(Type::Void)
                    }).collect();
                    let resolved_ret = if mname.name == "new" {
                        Type::Instance { name: concrete_name.clone() }
                    } else {
                        return_type.as_ref().and_then(|t| c.resolve_type(t)).unwrap_or(Type::Void)
                    };
                    let func_type = Type::Func { params: resolved_params, ret: Box::new(resolved_ret) };
                    let fn_name_clone = fn_name.clone();
                    c.scope.insert(&fn_name, Symbol { name: fn_name_clone, kind: SymbolKind::Function, type_: func_type })
                        .unwrap_or_else(|e| c.error(name.span, e));
                }
            }
            // Also register fields as a scope entry so field access can work
            None
        }
        Stmt::Func { name, params, return_type, body } => {
            c.scope.push();
            for param in params {
                let t = param.type_annotation.as_ref().and_then(|t| c.resolve_type(t)).unwrap_or(Type::Void);
                c.scope.insert(&param.name.name, Symbol { name: param.name.name.clone(), kind: SymbolKind::Variable, type_: t })
                    .unwrap_or_else(|e| c.error(param.name.span, e));
            }
            let ret_type = return_type.as_ref().and_then(|t| c.resolve_type(t)).unwrap_or(Type::Void);
            c.in_function = true;
            c.expected_return = Some(ret_type.clone());
            super::expr::check_expr(c, body);
            c.in_function = false;
            c.expected_return = None;
            c.scope.pop();
            let func_type = Type::Func {
                params: params.iter().filter(|p| !p.vararg).map(|p| p.type_annotation.as_ref().and_then(|t| c.resolve_type(t)).unwrap_or(Type::Void)).collect(),
                ret: Box::new(ret_type),
            };
            c.scope.insert(&name.name, Symbol { name: name.name.clone(), kind: SymbolKind::Function, type_: func_type })
                .unwrap_or_else(|e| c.error(name.span, e));
            None
        }
        Stmt::Return { value } => {
            let val_type = value.as_ref().map(|v| super::expr::check_expr(c, v)).flatten();
            let span = value.as_ref().map(|v| v.span()).unwrap_or(azurite_lexer::Span::new(0, 0, 0, 0));
            if let Some(ref expected) = c.expected_return {
                match val_type {
                    Some(ref actual) if *expected != *actual => {
                        c.error(span, format!("expected '{}', got '{}'", expected, actual));
                    }
                    None if *expected != Type::Void => {
                        c.error(span, format!("expected return type '{}'", expected));
                    }
                    _ => {}
                }
            }
            val_type
        }
        Stmt::Break | Stmt::Continue => {
            if c.in_loop == 0 {
                c.error(azurite_lexer::Span::new(0, 0, 0, 0), "'break'/'continue' outside loop".to_string());
            }
            None
        }
        Stmt::If { condition, then_branch, else_branch } => {
            super::expr::check_expr(c, condition);
            super::expr::check_expr(c, then_branch);
            if let Some(el) = else_branch { super::expr::check_expr(c, el); }
            None
        }
        Stmt::While { condition, body } => {
            c.in_loop += 1;
            super::expr::check_expr(c, condition);
            super::expr::check_expr(c, body);
            c.in_loop -= 1;
            None
        }
        Stmt::For { name, iterable, body } => {
            super::expr::check_expr(c, iterable);
            c.scope.push();
            c.scope.insert(&name.name, Symbol { name: name.name.clone(), kind: SymbolKind::Variable, type_: Type::Int })
                .unwrap_or_else(|e| c.error(name.span, e));
            c.in_loop += 1;
            super::expr::check_expr(c, body);
            c.in_loop -= 1;
            c.scope.pop();
            None
        }
        Stmt::Expr(expr) => super::expr::check_expr(c, expr),
        Stmt::Destructure { names, value } => {
            let val_type = super::expr::check_expr(c, value);
            match val_type {
                Some(Type::Tuple(types)) => {
                    if types.len() != names.len() {
                        c.error(value.span(), format!("tuple has {} elements, but {} names given", types.len(), names.len()));
                    }
                    for (i, name) in names.iter().enumerate() {
                        let t = types.get(i).cloned().unwrap_or(Type::Void);
                        c.scope.insert(&name.name, Symbol { name: name.name.clone(), kind: SymbolKind::Variable, type_: t })
                            .unwrap_or_else(|e| c.error(name.span, e));
                    }
                    Some(Type::Void)
                }
                Some(other) => {
                    c.error(value.span(), format!("cannot destructure '{}'", other));
                    None
                }
                None => None,
            }
        }
    }
}
