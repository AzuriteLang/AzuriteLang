use azurite_errors::{AzError, ErrorKind};
use azurite_parser::ast::*;
use crate::symbol::{Scope, Symbol, SymbolKind};
use crate::types::Type;

pub struct Checker {
    scope: Scope,
    errors: Vec<AzError>,
    in_function: bool,
    expected_return: Option<Type>,
}

impl Checker {
    pub fn new() -> Self {
        let mut checker = Self {
            scope: Scope::new(),
            errors: Vec::new(),
            in_function: false,
            expected_return: None,
        };
        checker.register_builtins();
        checker
    }

    fn register_builtins(&mut self) {
        let builtins = [
            ("print", Type::Func { params: vec![Type::String], ret: Box::new(Type::Void) }),
            ("println", Type::Func { params: vec![Type::String], ret: Box::new(Type::Void) }),
            ("print_int", Type::Func { params: vec![Type::Int], ret: Box::new(Type::Void) }),
            ("len", Type::Func { params: vec![Type::String], ret: Box::new(Type::Int) }),
            ("int", Type::Func { params: vec![Type::Float], ret: Box::new(Type::Int) }),
            ("float", Type::Func { params: vec![Type::Int], ret: Box::new(Type::Float) }),
            ("sqrt", Type::Func { params: vec![Type::Float], ret: Box::new(Type::Float) }),
            ("abs", Type::Func { params: vec![Type::Int], ret: Box::new(Type::Int) }),
            ("read", Type::Func { params: vec![], ret: Box::new(Type::String) }),
            ("input", Type::Func { params: vec![Type::String], ret: Box::new(Type::String) }),
            ("exit", Type::Func { params: vec![Type::Int], ret: Box::new(Type::Void) }),
            ("to_string", Type::Func { params: vec![Type::Int], ret: Box::new(Type::String) }),
        ];
        for (name, type_) in builtins {
            self.scope.insert(name, Symbol {
                name: name.to_string(),
                kind: SymbolKind::Function,
                type_,
            }).ok();
        }
    }

    pub fn check_program(&mut self, program: &Program) -> Result<(), Vec<AzError>> {
        self.errors.clear();
        for stmt in &program.statements {
            self.check_stmt(stmt);
        }
        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(self.errors.clone())
        }
    }

    fn error(&mut self, span: azurite_lexer::Span, msg: impl Into<String>) {
        self.errors.push(AzError::new(ErrorKind::TypeError, span, msg));
    }

    fn check_stmt(&mut self, stmt: &Stmt) -> Option<Type> {
        match stmt {
            Stmt::Let { name, type_annotation, value } => {
                let inferred = self.check_expr(value);
                let declared = type_annotation.as_ref()
                    .and_then(|t| self.resolve_type(t));

                let type_ = match (inferred, declared) {
                    (Some(inf), Some(dec)) => {
                        if inf != dec {
                            self.error(name.span, format!(
                                "type mismatch: expected '{}', got '{}' in 'let {}'",
                                dec, inf, name.name
                            ));
                        }
                        Some(dec)
                    }
                    (Some(inf), None) => {
                        if inf == Type::Null {
                            self.error(name.span, format!(
                                "cannot infer type for 'let {}', add explicit type annotation",
                                name.name
                            ));
                        }
                        Some(inf)
                    }
                    (None, Some(dec)) => Some(dec),
                    (None, None) => None,
                };

                if let Some(ref t) = type_ {
                    self.scope.insert(&name.name, Symbol {
                        name: name.name.clone(),
                        kind: SymbolKind::Variable,
                        type_: t.clone(),
                    }).unwrap_or_else(|e| self.error(name.span, e));
                }
                type_
            }
            Stmt::Import { .. } => {
                None
            }
            Stmt::Enum { .. } => {
                None
            }
            Stmt::Class { methods, .. } => {
                for method in methods {
                    self.check_stmt(method);
                }
                None
            }
            Stmt::Func { name, params, return_type, body } => {
                self.scope.push();

                for param in params {
                    let type_ = param.type_annotation.as_ref()
                        .and_then(|t| self.resolve_type(t))
                        .unwrap_or(Type::Void);
                    self.scope.insert(&param.name.name, Symbol {
                        name: param.name.name.clone(),
                        kind: SymbolKind::Variable,
                        type_,
                    }).unwrap_or_else(|e| self.error(param.name.span, e));
                }

                let ret_type = return_type.as_ref()
                    .and_then(|t| self.resolve_type(t))
                    .unwrap_or(Type::Void);

                self.in_function = true;
                self.expected_return = Some(ret_type.clone());
                self.check_expr(body);
                self.in_function = false;
                self.expected_return = None;

                self.scope.pop();

                let func_type = Type::Func {
                    params: params.iter()
                        .map(|p| p.type_annotation.as_ref()
                            .and_then(|t| self.resolve_type(t))
                            .unwrap_or(Type::Void))
                        .collect(),
                    ret: Box::new(ret_type),
                };

                self.scope.insert(&name.name, Symbol {
                    name: name.name.clone(),
                    kind: SymbolKind::Function,
                    type_: func_type,
                }).unwrap_or_else(|e| self.error(name.span, e));

                None
            }
            Stmt::Return { value } => {
                let val_type = value.as_ref().map(|v| self.check_expr(v)).flatten();
                if let Some(ref expected) = self.expected_return {
                    match val_type {
                        Some(ref actual) if *expected != *actual => {
                            let span = value.as_ref().map_or(
                                azurite_lexer::Span::new(0, 0, 0, 0),
                                |v| self.expr_span(v),
                            );
                            self.error(span, format!(
                                "expected return type '{}', got '{}'",
                                expected, actual
                            ));
                        }
                        None if *expected != Type::Void => {
                            self.error(azurite_lexer::Span::new(0, 0, 0, 0), format!(
                                "expected return value of type '{}'",
                                expected
                            ));
                        }
                        _ => {}
                    }
                }
                val_type
            }
            Stmt::If { condition, then_branch, else_branch } => {
                self.check_expr(condition);
                self.check_expr(then_branch);
                if let Some(else_) = else_branch {
                    self.check_expr(else_);
                }
                None
            }
            Stmt::While { condition, body } => {
                self.check_expr(condition);
                self.check_expr(body);
                None
            }
            Stmt::For { name, iterable, body } => {
                self.check_expr(iterable);
                self.scope.push();
                self.scope.insert(&name.name, Symbol {
                    name: name.name.clone(),
                    kind: SymbolKind::Variable,
                    type_: Type::Int,
                }).unwrap_or_else(|e| self.error(name.span, e));
                self.check_expr(body);
                self.scope.pop();
                None
            }
            Stmt::Expr(expr) => self.check_expr(expr),
        }
    }

    fn check_expr(&mut self, expr: &Expr) -> Option<Type> {
        match expr {
            Expr::Int(_) => Some(Type::Int),
            Expr::Float(_) => Some(Type::Float),
            Expr::String(_) => Some(Type::String),
            Expr::Char(_) => Some(Type::Int),
            Expr::Bool(_) => Some(Type::Bool),
            Expr::Null => Some(Type::Null),
            Expr::Self_ => {
                // self type resolved from method context
                Some(Type::Void)
            }
            Expr::FieldAccess { obj, .. } => {
                self.check_expr(obj)
            }
            Expr::MethodCall { obj, args, .. } => {
                self.check_expr(obj);
                for arg in args { self.check_expr(arg); }
                Some(Type::Void)
            }
            Expr::EnumVariant { args, .. } => {
                for arg in args { self.check_expr(arg); }
                Some(Type::Void)
            }
            Expr::Array(elems) => {
                for e in elems { self.check_expr(e); }
                Some(Type::Void)
            }
            Expr::Index { obj, index } => {
                self.check_expr(obj);
                self.check_expr(index);
                Some(Type::Void)
            }
            Expr::Ident(ident) => {
                match self.scope.lookup(&ident.name) {
                    Some(sym) => Some(sym.type_.clone()),
                    None => {
                        self.error(ident.span, format!("undefined variable '{}'", ident.name));
                        None
                    }
                }
            }
            Expr::Binary { left, op, right } => {
                let l = self.check_expr(left);
                let r = self.check_expr(right);
                self.check_binary_op(l, r, *op)
            }
            Expr::Unary { op, operand } => {
                let t = self.check_expr(operand);
                self.check_unary_op(t, *op)
            }
            Expr::Call { callee, args } => {
                let callee_type = self.check_expr(callee);
                match callee_type {
                    Some(Type::Func { params, ret }) => {
                        if params.len() != args.len() {
                            self.error(azurite_lexer::Span::new(0, 0, 0, 0), format!(
                                "expected {} arguments, got {}",
                                params.len(), args.len()
                            ));
                        }
                        for (i, arg) in args.iter().enumerate() {
                            let arg_type = self.check_expr(arg);
                            if let (Some(expected), Some(actual)) = (params.get(i), arg_type) {
                                if expected != &actual {
                                    self.error(azurite_lexer::Span::new(0, 0, 0, 0), format!(
                                        "argument {}: expected '{}', got '{}'",
                                        i + 1, expected, actual
                                    ));
                                }
                            }
                        }
                        Some(*ret)
                    }
                    Some(other) => {
                        self.error(azurite_lexer::Span::new(0, 0, 0, 0), format!(
                            "cannot call non-function type '{}'", other
                        ));
                        None
                    }
                    None => None,
                }
            }
            Expr::Block(stmts) => {
                self.scope.push();
                let mut last_type = None;
                for stmt in stmts {
                    last_type = self.check_stmt(stmt);
                }
                self.scope.pop();
                last_type
            }
            Expr::If { condition, then_branch, else_branch } => {
                self.check_expr(condition);
                let t = self.check_expr(then_branch);
                let e = else_branch.as_ref().map(|b| self.check_expr(b)).flatten();
                t.or(e)
            }
            Expr::While { condition, body } => {
                self.check_expr(condition);
                self.check_expr(body);
                Some(Type::Void)
            }
            Expr::Match { value, arms } => {
                self.check_expr(value);
                for arm in arms { self.check_expr(&arm.body); }
                Some(Type::Void)
            }
            Expr::Range { start, end } => {
                self.check_expr(start);
                self.check_expr(end);
                Some(Type::Void)
            }
        }
    }

    fn expr_span(&self, expr: &Expr) -> azurite_lexer::Span {
        match expr {
            Expr::Ident(ident) => ident.span,
            Expr::Int(_) | Expr::Float(_) | Expr::String(_) | Expr::Char(_)
                | Expr::Bool(_) | Expr::Null => azurite_lexer::Span::new(0, 0, 0, 0),
            _ => azurite_lexer::Span::new(0, 0, 0, 0),
        }
    }

    fn check_binary_op(&mut self, l: Option<Type>, r: Option<Type>, op: BinOp) -> Option<Type> {
        let lt = l.clone().unwrap_or(Type::Void);
        let rt = r.clone().unwrap_or(Type::Void);

        match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
                if lt.is_numeric() && rt.is_numeric() {
                    Some(if lt == Type::Float || rt == Type::Float { Type::Float } else { Type::Int })
                } else if op == BinOp::Add && lt == Type::String && rt == Type::String {
                    Some(Type::String)
                } else {
                    self.error(azurite_lexer::Span::new(0, 0, 0, 0), format!("cannot apply '{}' to '{}' and '{}'", op, lt, rt));
                    None
                }
            }
            BinOp::Eq | BinOp::Neq => {
                if lt == rt || (lt.is_numeric() && rt.is_numeric()) {
                    Some(Type::Bool)
                } else {
                    self.error(azurite_lexer::Span::new(0, 0, 0, 0), format!("cannot compare '{}' with '{}'", lt, rt));
                    None
                }
            }
            BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => {
                if lt.is_numeric() && rt.is_numeric() {
                    Some(Type::Bool)
                } else {
                    self.error(azurite_lexer::Span::new(0, 0, 0, 0), format!("cannot compare '{}' with '{}'", lt, rt));
                    None
                }
            }
            BinOp::And | BinOp::Or => {
                if lt == Type::Bool && rt == Type::Bool {
                    Some(Type::Bool)
                } else {
                    self.error(azurite_lexer::Span::new(0, 0, 0, 0), format!("cannot apply '{}' to '{}' and '{}'", op, lt, rt));
                    None
                }
            }
            BinOp::Assign => {
                if lt == rt || lt == Type::Null {
                    r
                } else {
                    self.error(azurite_lexer::Span::new(0, 0, 0, 0), format!("cannot assign '{}' to '{}'", rt, lt));
                    None
                }
            }
            BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::Shl | BinOp::Shr => {
                if lt == Type::Int && rt == Type::Int {
                    Some(Type::Int)
                } else {
                    self.error(azurite_lexer::Span::new(0, 0, 0, 0), format!("bitwise op requires ints, got '{}' and '{}'", lt, rt));
                    None
                }
            }
        }
    }

    fn check_unary_op(&mut self, t: Option<Type>, op: UnOp) -> Option<Type> {
        let t = t.unwrap_or(Type::Void);
        match op {
            UnOp::Neg if t.is_numeric() => Some(t),
            UnOp::Neg => {
                self.error(azurite_lexer::Span::new(0, 0, 0, 0), format!("cannot negate '{}'", t));
                None
            }
            UnOp::Not if t == Type::Bool => Some(Type::Bool),
            UnOp::Not => {
                self.error(azurite_lexer::Span::new(0, 0, 0, 0), format!("cannot apply 'not' to '{}'", t));
                None
            }
        }
    }

    fn resolve_type(&self, type_: &azurite_parser::ast::Type) -> Option<Type> {
        match type_ {
            azurite_parser::ast::Type::Name(name) => crate::types::Type::from_name(name),
            _ => None,
        }
    }
}
