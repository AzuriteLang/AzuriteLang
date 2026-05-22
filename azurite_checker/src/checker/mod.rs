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
    pub expected_return: Option<Type>,
    pub generic_classes: std::collections::HashMap<String, (Vec<String>, Vec<ClassField>, Vec<Stmt>)>,
}

impl Checker {
    pub fn new() -> Self {
        let mut checker = Self {
            scope: Scope::new(),
            errors: Vec::new(),
            in_function: false,
            expected_return: None,
            generic_classes: std::collections::HashMap::new(),
        };
        checker.register_builtins();
        checker
    }

    fn register_builtins(&mut self) {
        let builtins = [
            ("print", Type::Func { params: vec![], ret: Box::new(Type::Void) }),
            ("len", Type::Func { params: vec![Type::String], ret: Box::new(Type::Int) }),
            ("int", Type::Func { params: vec![Type::Float], ret: Box::new(Type::Int) }),
            ("float", Type::Func { params: vec![Type::Int], ret: Box::new(Type::Float) }),
            ("sqrt", Type::Func { params: vec![Type::Float], ret: Box::new(Type::Float) }),
            ("abs", Type::Func { params: vec![Type::Int], ret: Box::new(Type::Int) }),
            ("read", Type::Func { params: vec![], ret: Box::new(Type::String) }),
            ("input", Type::Func { params: vec![Type::String], ret: Box::new(Type::String) }),
            ("exit", Type::Func { params: vec![Type::Int], ret: Box::new(Type::Void) }),
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
            azurite_parser::ast::Type::Name(name) => crate::types::Type::from_name(name),
            azurite_parser::ast::Type::Generic { name, params } => {
                if let Some((_tp, _fl, methods)) = self.generic_classes.get(name).cloned() {
                    let cn = format!("{}_{}", name, params.iter().map(|p| {
                        match p { azurite_parser::ast::Type::Name(n) => n.clone(), _ => "any".to_string() }
                    }).collect::<Vec<_>>().join("_"));
                    for method in &methods {
                        if let Stmt::Func { name: mname, .. } = method {
                            let fn_name = format!("{}_{}", cn, mname.name);
                            let sym = Symbol { name: fn_name.clone(), kind: SymbolKind::Function, type_: Type::Func { params: vec![], ret: Box::new(Type::Void) } };
                            self.scope.insert(&fn_name, sym).ok();
                        }
                    }
                    crate::types::Type::from_name(&cn).or(Some(Type::Void))
                } else { None }
            }
            _ => None,
        }
    }
}
