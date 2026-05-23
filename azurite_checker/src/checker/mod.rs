use std::collections::HashMap;
use azurite_errors::{AzError, ErrorKind};
use azurite_parser::ast::*;
use crate::symbol::{Scope, Symbol, SymbolKind};
use crate::types::Type;

pub mod stmt;
pub mod expr;

pub struct Checker {
    pub scope: Scope,
    pub errors: Vec<AzError>,
    pub in_function: bool,
    pub in_loop: usize,
    pub expected_return: Option<Type>,
    pub generic_classes: HashMap<String, (Vec<String>, Vec<ClassField>, Vec<Stmt>)>,
    pub concrete_classes: HashMap<String, Vec<ClassField>>,
    pub enums: HashMap<String, Vec<EnumVariant>>,
}

impl Checker {
    pub fn new() -> Self {
        let mut checker = Self {
            scope: Scope::new(),
            errors: Vec::new(),
            in_function: false,
            in_loop: 0,
            expected_return: None,
            generic_classes: HashMap::new(),
            concrete_classes: HashMap::new(),
            enums: HashMap::new(),
        };
        checker.register_builtins();
        checker
    }

    fn register_builtins(&mut self) {
        let builtins = [
            ("print", Type::Func { params: vec![], ret: Box::new(Type::Void) }), // varargs (handled by codegen)
            ("len", Type::Func { params: vec![Type::String], ret: Box::new(Type::Int) }),
            ("int", Type::Func { params: vec![Type::Float], ret: Box::new(Type::Int) }),
            ("float", Type::Func { params: vec![Type::Int], ret: Box::new(Type::Float) }),
            ("sqrt", Type::Func { params: vec![Type::Float], ret: Box::new(Type::Float) }),
            ("abs", Type::Func { params: vec![Type::Int], ret: Box::new(Type::Int) }),
            ("read", Type::Func { params: vec![], ret: Box::new(Type::String) }),
            ("input", Type::Func { params: vec![Type::String], ret: Box::new(Type::String) }),
            ("exit", Type::Func { params: vec![Type::Int], ret: Box::new(Type::Void) }),
            ("char_at", Type::Func { params: vec![Type::String, Type::Int], ret: Box::new(Type::Int) }),
            ("chr", Type::Func { params: vec![Type::Int], ret: Box::new(Type::String) }),
            ("sin", Type::Func { params: vec![Type::Float], ret: Box::new(Type::Float) }),
            ("cos", Type::Func { params: vec![Type::Float], ret: Box::new(Type::Float) }),
            ("tan", Type::Func { params: vec![Type::Float], ret: Box::new(Type::Float) }),
            ("pow", Type::Func { params: vec![Type::Float, Type::Float], ret: Box::new(Type::Float) }),
            ("log", Type::Func { params: vec![Type::Float], ret: Box::new(Type::Float) }),
            ("log10", Type::Func { params: vec![Type::Float], ret: Box::new(Type::Float) }),
            ("floor", Type::Func { params: vec![Type::Float], ret: Box::new(Type::Float) }),
            ("ceil", Type::Func { params: vec![Type::Float], ret: Box::new(Type::Float) }),
            ("asin", Type::Func { params: vec![Type::Float], ret: Box::new(Type::Float) }),
            ("acos", Type::Func { params: vec![Type::Float], ret: Box::new(Type::Float) }),
            ("atan", Type::Func { params: vec![Type::Float], ret: Box::new(Type::Float) }),
            ("atan2", Type::Func { params: vec![Type::Float, Type::Float], ret: Box::new(Type::Float) }),
            ("sinh", Type::Func { params: vec![Type::Float], ret: Box::new(Type::Float) }),
            ("cosh", Type::Func { params: vec![Type::Float], ret: Box::new(Type::Float) }),
            ("tanh", Type::Func { params: vec![Type::Float], ret: Box::new(Type::Float) }),
            ("exp", Type::Func { params: vec![Type::Float], ret: Box::new(Type::Float) }),
            ("expm1", Type::Func { params: vec![Type::Float], ret: Box::new(Type::Float) }),
            ("log2", Type::Func { params: vec![Type::Float], ret: Box::new(Type::Float) }),
            ("hypot", Type::Func { params: vec![Type::Float, Type::Float], ret: Box::new(Type::Float) }),
            ("fmod", Type::Func { params: vec![Type::Float, Type::Float], ret: Box::new(Type::Float) }),
            ("copysign", Type::Func { params: vec![Type::Float, Type::Float], ret: Box::new(Type::Float) }),
            ("rand", Type::Func { params: vec![], ret: Box::new(Type::Int) }),
            ("srand", Type::Func { params: vec![Type::Int], ret: Box::new(Type::Void) }),
        ];
        for (name, type_) in builtins {
            self.scope.insert(name, Symbol {
                name: name.to_string(), kind: SymbolKind::Function, type_,
            }).ok();
        }
    }

    pub fn check_program(&mut self, program: &Program) -> Result<(), Vec<AzError>> {
        self.errors.clear();
        for stmt in &program.statements {
            stmt::check_stmt(self, stmt);
        }
        if self.errors.is_empty() { Ok(()) } else { Err(self.errors.clone()) }
    }

    pub fn error(&mut self, span: azurite_lexer::Span, msg: impl Into<String>) {
        self.errors.push(AzError::new(ErrorKind::TypeError, span, msg));
    }

    pub fn resolve_type(&mut self, type_: &azurite_parser::ast::Type) -> Option<Type> {
        match type_ {
            azurite_parser::ast::Type::Name(ref name) => {
                if self.concrete_classes.contains_key(name) {
                    Some(Type::Instance { name: name.clone() })
                } else {
                    crate::types::Type::from_name(name)
                }
            }
            azurite_parser::ast::Type::Generic { name, params } => {
                self.create_concrete_from_generic(name, params)
            }
            azurite_parser::ast::Type::Tuple(types) => {
                let resolved: Vec<Type> = types.iter().filter_map(|t| self.resolve_type(t)).collect();
                if resolved.len() == types.len() { Some(Type::Tuple(resolved)) } else { None }
            }
            _ => None,
        }
    }

    pub fn subst_ast_type(&self, ty: &azurite_parser::ast::Type, type_params: &[String], concrete_types: &[Type]) -> azurite_parser::ast::Type {
        match ty {
            azurite_parser::ast::Type::Name(n) => {
                if let Some(idx) = type_params.iter().position(|p| p == n) {
                    let ct = &concrete_types[idx];
                    azurite_parser::ast::Type::Name(match ct {
                        Type::Int => "int".to_string(),
                        Type::Float => "float".to_string(),
                        Type::String => "string".to_string(),
                        Type::Bool => "bool".to_string(),
                        Type::Instance { name } => name.clone(),
                        _ => "int".to_string(),
                    })
                } else {
                    ty.clone()
                }
            }
            azurite_parser::ast::Type::Generic { name, params } => {
                azurite_parser::ast::Type::Generic {
                    name: name.clone(),
                    params: params.iter().map(|p| self.subst_ast_type(p, type_params, concrete_types)).collect(),
                }
            }
            _ => ty.clone(),
        }
    }

    pub fn create_concrete_from_generic(&mut self, base_name: &str, params: &[azurite_parser::ast::Type]) -> Option<Type> {
        let entry = self.generic_classes.get(base_name)?.clone();
        let (type_params, fields, methods) = entry;
        let concrete_types: Vec<Type> = params.iter().map(|p| {
            match p {
                azurite_parser::ast::Type::Name(n) => Type::from_name(n).unwrap_or(Type::Int),
                _ => Type::Int,
            }
        }).collect();
        let suffix: Vec<String> = concrete_types.iter().map(|t| t.to_string()).collect();
        let concrete_name = format!("{}_{}", base_name, suffix.join("_"));
        if self.concrete_classes.contains_key(&concrete_name) {
            return Some(Type::Instance { name: concrete_name });
        }
        let concrete_fields: Vec<ClassField> = fields.iter().map(|f| ClassField {
            name: f.name.clone(),
            type_: self.subst_ast_type(&f.type_, &type_params, &concrete_types),
        }).collect();
        self.concrete_classes.insert(concrete_name.clone(), concrete_fields);
        for method in &methods {
            if let Stmt::Func { name: mname, params: mparams, return_type, .. } = method {
                let fn_name = format!("{}_{}", concrete_name, mname.name);
                let resolved_params: Vec<Type> = mparams.iter().filter(|p| p.name.name != "self").map(|p| {
                    p.type_annotation.as_ref()
                        .map(|ta| self.resolve_type(&self.subst_ast_type(ta, &type_params, &concrete_types)))
                        .flatten()
                        .unwrap_or(Type::Void)
                }).collect();
                let resolved_ret = return_type.as_ref()
                    .map(|rt| self.resolve_type(&self.subst_ast_type(rt, &type_params, &concrete_types)))
                    .flatten()
                    .unwrap_or(Type::Void);
                let func_type = Type::Func { params: resolved_params, ret: Box::new(resolved_ret) };
                let fn_name_clone = fn_name.clone();
                self.scope.insert(&fn_name, Symbol { name: fn_name_clone, kind: SymbolKind::Function, type_: func_type }).ok();
            }
        }
        Some(Type::Instance { name: concrete_name })
    }

    pub fn instantiate_generic_constructor(&mut self, class_name: &str, args: &[Expr]) -> Option<Type> {
        let (type_params, _, _) = self.generic_classes.get(class_name)?.clone();
        let concrete_types: Vec<Type> = type_params.iter().enumerate().map(|(i, _tp)| {
            args.get(i).map(|a| expr::check_expr(self, a)).flatten().unwrap_or(Type::Int)
        }).collect();
        let fake_ast_params: Vec<azurite_parser::ast::Type> = concrete_types.iter().map(|ct| {
            azurite_parser::ast::Type::Name(match ct {
                Type::Int => "int".to_string(),
                Type::Float => "float".to_string(),
                Type::String => "string".to_string(),
                Type::Bool => "bool".to_string(),
                _ => "int".to_string(),
            })
        }).collect();
        drop(type_params); // release borrow
        self.create_concrete_from_generic(class_name, &fake_ast_params)
    }
}
