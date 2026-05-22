pub mod expr;
pub mod class;
pub mod builtin;

use std::collections::HashMap;

use azurite_errors::{AzError, ErrorKind};
use azurite_lexer::Span;
use azurite_parser::ast::*;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::types::{BasicMetadataTypeEnum, BasicType, BasicTypeEnum};
use inkwell::values::{BasicValueEnum, FunctionValue, IntValue, PointerValue};
use inkwell::IntPredicate;

pub struct ClassInfo<'ctx> {
    pub field_names: Vec<String>,
    pub field_types: Vec<BasicTypeEnum<'ctx>>,
    pub methods: Vec<String>,
    pub llvm_struct: inkwell::types::StructType<'ctx>,
}

pub struct CodeGen<'ctx> {
    pub context: &'ctx Context,
    pub module: Module<'ctx>,
    pub builder: Builder<'ctx>,
    pub variables: HashMap<String, (PointerValue<'ctx>, BasicTypeEnum<'ctx>)>,
    pub struct_types: HashMap<String, ClassInfo<'ctx>>,
    pub function: Option<FunctionValue<'ctx>>,
    pub self_ptr: Option<PointerValue<'ctx>>,
    pub current_class: Option<String>,
    pub printf: Option<FunctionValue<'ctx>>,
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
            struct_types: HashMap::new(),
            function: None,
            self_ptr: None,
            current_class: None,
            printf: None,
        }
    }

    pub fn module(&self) -> &Module<'ctx> { &self.module }

    pub fn compile_program(&mut self, program: &Program) -> Result<(), AzError> {
        for stmt in &program.statements {
            self.compile_stmt(stmt, false)?;
        }
        Ok(())
    }

    pub fn compile_stmt(&mut self, stmt: &Stmt, _is_tail: bool) -> Result<Option<BasicValueEnum<'ctx>>, AzError> {
        match stmt {
            Stmt::Let { name, value, .. } => {
                let val = self.compile_expr(value)?;
                let val_type = val.get_type();
                let alloca = self.create_entry_alloca(val_type, &name.name);
                self.builder.build_store(alloca, val).unwrap();
                self.variables.insert(name.name.clone(), (alloca, val_type));
                Ok(Some(val))
            }
            Stmt::Func { name, params, return_type, body } => {
                let is_void = return_type.is_none() || matches!(return_type, Some(azurite_parser::ast::Type::Name(ref n)) if n == "void" || n == "none");

                let param_types: Vec<BasicMetadataTypeEnum> = params.iter()
                    .map(|p| self.az_param_type(&p.type_annotation))
                    .collect();

                let fn_val = if is_void {
                    let ft = self.context.void_type().fn_type(&param_types, false);
                    self.module.add_function(&name.name, ft, None)
                } else {
                    let ft = self.context.i64_type().fn_type(&param_types, false);
                    self.module.add_function(&name.name, ft, None)
                };

                let entry = self.context.append_basic_block(fn_val, "entry");
                self.builder.position_at_end(entry);
                self.function = Some(fn_val);

                for (i, param) in params.iter().enumerate() {
                    if let Some(pv) = fn_val.get_nth_param(i as u32) {
                        let ptr = self.create_entry_alloca(pv.get_type(), &param.name.name);
                        self.builder.build_store(ptr, pv).unwrap();
                        self.variables.insert(param.name.name.clone(), (ptr, pv.get_type()));
                    }
                }

                let last_val = self.compile_block_stmts(body, true)?;

                if !self.has_terminator() {
                    if is_void {
                        self.builder.build_return(None).unwrap();
                    } else if let Some(v) = last_val {
                        self.builder.build_return(Some(&self.any_to_i64(v))).unwrap();
                    } else {
                        self.builder.build_return(Some(&self.context.i64_type().const_zero())).unwrap();
                    }
                }

                self.function = None;
                Ok(None)
            }
            Stmt::Class { name, fields, methods } => {
                class::compile_class(self, name, fields, methods)?;
                Ok(None)
            }
            Stmt::Import { .. } | Stmt::Enum { .. } => {
                Ok(None)
            }
            Stmt::For { name, iterable, body } => {
                let cf = self.function.unwrap();

                // Check for range: for i in 0..10
                // The iterable should produce start and end values
                let start: BasicValueEnum = self.context.i64_type().const_zero().into();
                let end: BasicValueEnum = self.context.i64_type().const_int(10, false).into();

                let i_ptr = self.create_entry_alloca(self.context.i64_type().into(), &name.name);
                self.builder.build_store(i_ptr, start).unwrap();
                self.variables.insert(name.name.clone(), (i_ptr, self.context.i64_type().into()));

                let cond_bb = self.context.append_basic_block(cf, "for_cond");
                let body_bb = self.context.append_basic_block(cf, "for_body");
                let after_bb = self.context.append_basic_block(cf, "for_after");

                self.builder.build_unconditional_branch(cond_bb).unwrap();
                self.builder.position_at_end(cond_bb);

                let i_val = self.builder.build_load(self.context.i64_type(), i_ptr, "i").unwrap();
                let end_i = match end {
                    BasicValueEnum::IntValue(v) => v,
                    _ => self.context.i64_type().const_int(10, false),
                };
                let cmp = self.builder.build_int_compare(
                    inkwell::IntPredicate::SLT, i_val.into_int_value(), end_i, "forcmp",
                ).unwrap();
                self.builder.build_conditional_branch(cmp, body_bb, after_bb).unwrap();

                self.builder.position_at_end(body_bb);
                self.compile_block_stmts(body, false)?;

                let i_next = self.builder.build_load(self.context.i64_type(), i_ptr, "i").unwrap();
                let one = self.context.i64_type().const_int(1, false);
                let i_inc = self.builder.build_int_add(i_next.into_int_value(), one, "iinc").unwrap();
                self.builder.build_store(i_ptr, i_inc).unwrap();

                self.builder.build_unconditional_branch(cond_bb).unwrap();
                self.builder.position_at_end(after_bb);
                Ok(Some(self.context.i64_type().const_zero().into()))
            }
            Stmt::Return { value } => {
                if let Some(v) = value {
                    let compiled = self.compile_expr(v)?;
                    self.builder.build_return(Some(&compiled)).unwrap();
                    Ok(Some(compiled))
                } else {
                    self.builder.build_return(None).unwrap();
                    Ok(None)
                }
            }
            Stmt::Expr(expr) => {
                let val = self.compile_expr(expr)?;
                Ok(Some(val))
            }
            Stmt::If { condition, then_branch, else_branch } => {
                let cond = self.compile_expr(condition)?;
                let cond_int = self.to_bool(cond);
                let cf = self.function.unwrap();
                let then_bb = self.context.append_basic_block(cf, "then");
                let else_bb = self.context.append_basic_block(cf, "else");
                let merge_bb = self.context.append_basic_block(cf, "ifcont");
                self.builder.build_conditional_branch(cond_int, then_bb, else_bb).unwrap();
                self.builder.position_at_end(then_bb);
                self.compile_block_stmts(then_branch, false)?;
                self.builder.build_unconditional_branch(merge_bb).unwrap();
                self.builder.position_at_end(else_bb);
                if let Some(eb) = else_branch { self.compile_block_stmts(eb, false)?; }
                self.builder.build_unconditional_branch(merge_bb).unwrap();
                self.builder.position_at_end(merge_bb);
                Ok(Some(self.context.i64_type().const_zero().into()))
            }
            Stmt::While { condition, body } => {
                let cf = self.function.unwrap();
                let cond_bb = self.context.append_basic_block(cf, "while_cond");
                let body_bb = self.context.append_basic_block(cf, "while_body");
                let after_bb = self.context.append_basic_block(cf, "while_after");
                self.builder.build_unconditional_branch(cond_bb).unwrap();
                self.builder.position_at_end(cond_bb);
                let cond = self.compile_expr(condition)?;
                let cond_int = self.to_bool(cond);
                self.builder.build_conditional_branch(cond_int, body_bb, after_bb).unwrap();
                self.builder.position_at_end(body_bb);
                self.compile_block_stmts(body, false)?;
                self.builder.build_unconditional_branch(cond_bb).unwrap();
                self.builder.position_at_end(after_bb);
                Ok(Some(self.context.i64_type().const_zero().into()))
            }
        }
    }

    pub fn compile_block_stmts(&mut self, expr: &Expr, tail: bool) -> Result<Option<BasicValueEnum<'ctx>>, AzError> {
        match expr {
            Expr::Block(stmts) => {
                let mut last = None;
                for (i, stmt) in stmts.iter().enumerate() {
                    let t = tail && i == stmts.len() - 1;
                    last = self.compile_stmt(stmt, t)?.or(last);
                }
                Ok(last)
            }
            _ => {
                if tail { let v = self.compile_expr(expr)?; Ok(Some(v)) }
                else { self.compile_expr(expr)?; Ok(None) }
            }
        }
    }

    pub fn compile_expr(&mut self, expr: &Expr) -> Result<BasicValueEnum<'ctx>, AzError> {
        expr::compile_expr(self, expr)
    }

    pub fn create_entry_alloca(&self, ty: BasicTypeEnum<'ctx>, name: &str) -> PointerValue<'ctx> {
        let entry = self.function.unwrap().get_first_basic_block().unwrap();
        let saved = self.builder.get_insert_block().unwrap();
        self.builder.position_at_end(entry);
        let alloca = self.builder.build_alloca(ty, name).unwrap();
        self.builder.position_at_end(saved);
        alloca
    }

    pub fn has_terminator(&self) -> bool {
        self.builder.get_insert_block()
            .and_then(|b| b.get_last_instruction())
            .is_some()
    }

    pub fn to_bool(&self, val: BasicValueEnum<'ctx>) -> IntValue<'ctx> {
        match val {
            BasicValueEnum::IntValue(i) => {
                let zero = self.context.i64_type().const_zero();
                self.builder.build_int_compare(IntPredicate::NE, i, zero, "booltmp").unwrap()
            }
            BasicValueEnum::FloatValue(f) => {
                let zero = self.context.f64_type().const_float(0.0);
                let cmp = self.builder.build_float_compare(inkwell::FloatPredicate::ONE, f, zero, "booltmp").unwrap();
                self.builder.build_int_z_extend(cmp, self.context.i64_type(), "booltmp").unwrap()
            }
            v => v.into_int_value(),
        }
    }

    pub fn any_to_i64(&self, val: BasicValueEnum<'ctx>) -> BasicValueEnum<'ctx> {
        match val {
            BasicValueEnum::IntValue(i) => {
                let i64 = self.context.i64_type();
                if i.get_type() == i64 { val }
                else { self.builder.build_int_z_extend(i, i64, "zext").unwrap().into() }
            }
            BasicValueEnum::FloatValue(f) => {
                self.builder.build_float_to_signed_int(f, self.context.i64_type(), "f2i").unwrap().into()
            }
            _ => self.context.i64_type().const_zero().into(),
        }
    }

    pub fn az_param_type(&self, type_: &Option<azurite_parser::ast::Type>) -> BasicMetadataTypeEnum<'ctx> {
        match type_ {
            Some(azurite_parser::ast::Type::Name(n)) if n == "string" => self.context.ptr_type(inkwell::AddressSpace::default()).into(),
            Some(azurite_parser::ast::Type::Name(n)) if n == "int" => self.context.i64_type().into(),
            Some(azurite_parser::ast::Type::Name(n)) if n == "float" => self.context.f64_type().into(),
            Some(azurite_parser::ast::Type::Name(n)) if n == "bool" => self.context.i64_type().into(),
            _ => self.context.i64_type().into(),
        }
    }

    pub fn field_type_to_llvm(&self, type_: &azurite_parser::ast::Type) -> BasicTypeEnum<'ctx> {
        match type_ {
            azurite_parser::ast::Type::Name(n) if n == "string" => self.context.ptr_type(inkwell::AddressSpace::default()).into(),
            azurite_parser::ast::Type::Name(n) if n == "int" => self.context.i64_type().into(),
            azurite_parser::ast::Type::Name(n) if n == "float" => self.context.f64_type().into(),
            azurite_parser::ast::Type::Name(n) if n == "bool" => self.context.i64_type().into(),
            _ => self.context.i64_type().into(),
        }
    }
}
