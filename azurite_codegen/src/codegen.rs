use azurite_parser::ast::*;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::builder::Builder;
use inkwell::values::AnyValueEnum;
use inkwell::IntValue;
use std::collections::HashMap;

pub struct CodeGen<'ctx> {
    context: &'ctx Context,
    module: Module<'ctx>,
    builder: Builder<'ctx>,
    variables: HashMap<String, AnyValueEnum<'ctx>>,
}

impl<'ctx> CodeGen<'ctx> {
    pub fn new(context: &'ctx Context, module_name: &str) -> Self {
        let module = context.create_module(module_name);
        let builder = context.create_builder();
        Self {
            context,
            module,
            builder,
            variables: HashMap::new(),
        }
    }

    pub fn module(&self) -> &Module<'ctx> {
        &self.module
    }

    pub fn compile_program(&mut self, program: &Program) -> Result<(), String> {
        for stmt in &program.statements {
            self.compile_stmt(stmt)?;
        }
        Ok(())
    }

    fn compile_stmt(&mut self, stmt: &Stmt) -> Result<AnyValueEnum<'ctx>, String> {
        match stmt {
            Stmt::Let { name, value, .. } => {
                let val = self.compile_expr(value)?;
                self.variables.insert(name.name.clone(), val);
                Ok(val)
            }
            Stmt::Expr(expr) => self.compile_expr(expr),
            Stmt::Return { value } => {
                if let Some(val) = value {
                    let compiled = self.compile_expr(val)?;
                    // TODO: return
                    Ok(compiled)
                } else {
                    Ok(self.context.i32_type().zero().into())
                }
            }
            _ => todo!("statement compilation not yet implemented: {:?}", stmt),
        }
    }

    fn compile_expr(&mut self, expr: &Expr) -> Result<AnyValueEnum<'ctx>, String> {
        match expr {
            Expr::Int(n) => {
                Ok(self.context.i64_type().const_int(*n as u64, false).into())
            }
            Expr::Float(n) => {
                Ok(self.context.f64_type().const_float(*n).into())
            }
            Expr::Bool(b) => {
                Ok(self.context.bool_type().const_int(*b as u64, false).into())
            }
            Expr::Ident(ident) => {
                self.variables.get(&ident.name)
                    .cloned()
                    .ok_or_else(|| format!("undefined variable '{}'", ident.name))
            }
            Expr::Binary { left, op, right } => {
                let lhs = self.compile_expr(left)?;
                let rhs = self.compile_expr(right)?;
                self.compile_binary(lhs, rhs, *op)
            }
            Expr::Block(stmts) => {
                let mut last = None;
                for stmt in stmts {
                    last = Some(self.compile_stmt(stmt)?);
                }
                last.ok_or_else(|| "empty block".to_string())
            }
            _ => todo!("expression compilation not yet implemented: {:?}", expr),
        }
    }

    fn compile_binary(
        &self,
        lhs: AnyValueEnum<'ctx>,
        rhs: AnyValueEnum<'ctx>,
        op: BinOp,
    ) -> Result<AnyValueEnum<'ctx>, String> {
        match (lhs, rhs) {
            (AnyValueEnum::IntValue(l), AnyValueEnum::IntValue(r)) => {
                let val = match op {
                    BinOp::Add => l.const_add(r).into(),
                    BinOp::Sub => l.const_sub(r).into(),
                    BinOp::Mul => l.const_mul(r).into(),
                    _ => return Err(format!("unsupported binary op for ints: {:?}", op)),
                };
                Ok(val)
            }
            (AnyValueEnum::FloatValue(l), AnyValueEnum::FloatValue(r)) => {
                let val = match op {
                    BinOp::Add => l.const_add(r).into(),
                    BinOp::Sub => l.const_sub(r).into(),
                    BinOp::Mul => l.const_mul(r).into(),
                    _ => return Err(format!("unsupported binary op for floats: {:?}", op)),
                };
                Ok(val)
            }
            _ => Err("type mismatch in binary operation".to_string()),
        }
    }
}
