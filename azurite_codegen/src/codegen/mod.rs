pub mod expr;
pub mod class;
pub mod builtin;

use std::collections::HashMap;

use azurite_errors::AzError;
use azurite_parser::ast::*;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::types::{BasicMetadataTypeEnum, BasicTypeEnum};
use inkwell::values::{BasicValueEnum, FunctionValue, IntValue, PointerValue};
use inkwell::IntPredicate;
use inkwell::basic_block::BasicBlock;

pub struct ClassInfo<'ctx> {
    pub field_names: Vec<String>,
    pub field_types: Vec<BasicTypeEnum<'ctx>>,
    pub methods: Vec<String>,
    pub llvm_struct: inkwell::types::StructType<'ctx>,
    pub parent: Option<String>,
    pub has_vtable: bool,
}

pub struct CodeGen<'ctx> {
    pub context: &'ctx Context,
    pub module: Module<'ctx>,
    pub builder: Builder<'ctx>,
    pub variables: HashMap<String, (PointerValue<'ctx>, BasicTypeEnum<'ctx>)>,
    pub struct_types: HashMap<String, ClassInfo<'ctx>>,
    pub generic_classes: HashMap<String, (Vec<String>, Vec<ClassField>, Vec<Stmt>)>,
    pub enums: HashMap<String, Vec<EnumVariant>>,
    pub function: Option<FunctionValue<'ctx>>,
    pub self_ptr: Option<PointerValue<'ctx>>,
    pub current_class: Option<String>,
    pub printf: Option<FunctionValue<'ctx>>,
    pub putchar: Option<FunctionValue<'ctx>>,
    pub loop_stack: Vec<(BasicBlock<'ctx>, BasicBlock<'ctx>)>,
    pub function_defaults: HashMap<String, Vec<Option<Box<Expr>>>>,
    pub jmp_buf: Option<PointerValue<'ctx>>,
    pub err_ptr: Option<PointerValue<'ctx>>,
    pub caught_flag: Option<PointerValue<'ctx>>,
    pub try_end_bb: Option<BasicBlock<'ctx>>,
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
            generic_classes: HashMap::new(),
            enums: HashMap::new(),
            function: None,
            self_ptr: None,
            current_class: None,
            printf: None,
            putchar: None,
            loop_stack: Vec::new(),
            function_defaults: HashMap::new(),
            jmp_buf: None,
            err_ptr: None,
            caught_flag: None,
            try_end_bb: None,
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
                // Store array length for literals
                if let Expr::Array(elems) = value.as_ref() {
                    let len_name = format!("{}.__len", name.name);
                    let len_alloca = self.create_entry_alloca(self.context.i64_type().into(), &len_name);
                    let len_val = self.context.i64_type().const_int(elems.len() as u64, false);
                    self.builder.build_store(len_alloca, len_val).unwrap();
                    self.variables.insert(len_name, (len_alloca, self.context.i64_type().into()));
                }
                Ok(Some(val))
            }
            Stmt::Func { name, params, return_type, body } => {
                let is_void = return_type.is_none() || matches!(return_type, Some(azurite_parser::ast::Type::Name(ref n)) if n == "void" || n == "none");
                let ret_is_string = matches!(return_type, Some(azurite_parser::ast::Type::Name(ref n)) if n == "string");
                let ret_is_float = matches!(return_type, Some(azurite_parser::ast::Type::Name(ref n)) if n == "float");
                let ret_is_tuple = matches!(return_type, Some(azurite_parser::ast::Type::Tuple(_)));
                let ret_name = return_type.as_ref().and_then(|t| if let azurite_parser::ast::Type::Name(n) = t { Some(n.as_str()) } else { None });
                let ret_is_instance = !is_void && !ret_is_string && !ret_is_float && !ret_is_tuple && ret_name.map_or(false, |n| n != "int" && n != "bool");

                let param_types: Vec<BasicMetadataTypeEnum> = params.iter()
                    .map(|p| self.az_param_type(&p.type_annotation))
                    .collect();
                let is_var_args = params.iter().any(|p| p.vararg);

                let fn_val = if is_void {
                    let ft = self.context.void_type().fn_type(&param_types, is_var_args);
                    self.module.add_function(&name.name, ft, None)
                } else if ret_is_string || ret_is_instance || ret_is_tuple {
                    let ft = self.context.ptr_type(inkwell::AddressSpace::default()).fn_type(&param_types, is_var_args);
                    self.module.add_function(&name.name, ft, None)
                } else if ret_is_float {
                    let ft = self.context.f64_type().fn_type(&param_types, is_var_args);
                    self.module.add_function(&name.name, ft, None)
                } else {
                    let ft = self.context.i64_type().fn_type(&param_types, is_var_args);
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
                        if ret_is_string || ret_is_instance || ret_is_tuple {
                            self.builder.build_return(Some(&v)).unwrap();
                        } else if ret_is_float {
                            match v {
                                BasicValueEnum::IntValue(i) => {
                                    let f: BasicValueEnum = self.builder.build_signed_int_to_float(i, self.context.f64_type(), "i2f").unwrap().into();
                                    self.builder.build_return(Some(&f)).unwrap();
                                }
                                _ => { self.builder.build_return(Some(&v)).unwrap(); }
                            }
                        } else {
                            self.builder.build_return(Some(&self.any_to_i64(v))).unwrap();
                        }
                    } else if ret_is_float {
                        let f0: BasicValueEnum = self.context.f64_type().const_float(0.0).into();
                        self.builder.build_return(Some(&f0)).unwrap();
                    } else if ret_is_instance || ret_is_tuple {
                        let null_ptr: BasicValueEnum = self.context.ptr_type(inkwell::AddressSpace::default()).const_zero().into();
                        self.builder.build_return(Some(&null_ptr)).unwrap();
                    } else {
                        self.builder.build_return(Some(&self.context.i64_type().const_zero())).unwrap();
                    }
                }

                self.function_defaults.insert(name.name.clone(), params.iter().map(|p| p.default_value.clone()).collect());
                self.function = None;
                Ok(None)
            }
            Stmt::Class { name, fields, methods, parent, type_params } => {
                if !type_params.is_empty() {
                    self.generic_classes.insert(name.name.clone(), (type_params.clone(), fields.clone(), methods.clone()));
                    return Ok(None);
                }
                class::compile_class(self, name, fields, methods, parent)?;
                Ok(None)
            }
            Stmt::Import { .. } => {
                Ok(None)
            }
            Stmt::Enum { name, variants } => {
                self.enums.insert(name.name.clone(), variants.clone());
                Ok(None)
            }
            Stmt::For { name, iterable, body } => {
                let cf = self.function.unwrap();
                let i64_ty = self.context.i64_type();

                match iterable.as_ref() {
                    Expr::Range { start, end } => {
                        let start_val = self.compile_expr(start)?.into_int_value();
                        let end_val = self.compile_expr(end)?.into_int_value();
                        let i_ptr = self.create_entry_alloca(i64_ty.into(), &name.name);
                        self.builder.build_store(i_ptr, start_val).unwrap();
                        self.variables.insert(name.name.clone(), (i_ptr, i64_ty.into()));

                        let cond_bb = self.context.append_basic_block(cf, "for_cond");
                        let body_bb = self.context.append_basic_block(cf, "for_body");
                        let inc_bb = self.context.append_basic_block(cf, "for_inc");
                        let after_bb = self.context.append_basic_block(cf, "for_after");
                        self.builder.build_unconditional_branch(cond_bb).unwrap();
                        self.builder.position_at_end(cond_bb);

                        let i_val = self.builder.build_load(i64_ty, i_ptr, "i").unwrap();
                        let cmp = self.builder.build_int_compare(
                            inkwell::IntPredicate::SLT, i_val.into_int_value(), end_val, "forcmp",
                        ).unwrap();
                        self.builder.build_conditional_branch(cmp, body_bb, after_bb).unwrap();
                        self.builder.position_at_end(body_bb);
                        self.loop_stack.push((inc_bb, after_bb));
                        self.compile_block_stmts(body, false)?;
                        self.loop_stack.pop();
                        if !self.has_terminator() { self.builder.build_unconditional_branch(inc_bb).unwrap(); }
                        self.builder.position_at_end(inc_bb);
                        let i_next = self.builder.build_load(i64_ty, i_ptr, "i").unwrap();
                        let one = i64_ty.const_int(1, false);
                        let i_inc = self.builder.build_int_add(i_next.into_int_value(), one, "iinc").unwrap();
                        self.builder.build_store(i_ptr, i_inc).unwrap();
                        self.builder.build_unconditional_branch(cond_bb).unwrap();
                        self.builder.position_at_end(after_bb);
                    }
                    // For each over array: for x in arr or for x in [1,2,3]
                    _ => {
                        let arr = self.compile_expr(iterable)?.into_pointer_value();
                        let (count, dyn_count) = match iterable.as_ref() {
                            Expr::Array(elems) => (elems.len() as i64, None),
                            Expr::Ident(ident) => {
                                let len_var = format!("{}.__len", ident.name);
                                if let Some((ptr, _)) = self.variables.get(&len_var) {
                                    (5i64, Some((*ptr, i64_ty)))
                                } else { (5i64, None) }
                            }
                            _ => (5i64, None),
                        };

                        // Use dynamic count from .__len if available
                        let limit_val = if let Some((lptr, lty)) = dyn_count {
                            self.builder.build_load(lty, lptr, "acnt").unwrap().into_int_value()
                        } else {
                            i64_ty.const_int(count as u64, false)
                        };

                        let i_ptr = self.create_entry_alloca(i64_ty.into(), &name.name);
                        self.builder.build_store(i_ptr, i64_ty.const_zero()).unwrap();

                        let cond_bb = self.context.append_basic_block(cf, "for_cond");
                        let body_bb = self.context.append_basic_block(cf, "for_body");
                        let inc_bb = self.context.append_basic_block(cf, "for_inc");
                        let after_bb = self.context.append_basic_block(cf, "for_after");

                        self.builder.build_unconditional_branch(cond_bb).unwrap();
                        self.builder.position_at_end(cond_bb);
                        let i = self.builder.build_load(i64_ty, i_ptr, "i").unwrap();
                        let cmp = self.builder.build_int_compare(
                            inkwell::IntPredicate::SLT, i.into_int_value(), limit_val, "fcmp",
                        ).unwrap();
                        self.builder.build_conditional_branch(cmp, body_bb, after_bb).unwrap();

                        self.builder.position_at_end(body_bb);
                        let elem = unsafe {
                            self.builder.build_gep(i64_ty, arr, &[self.builder.build_load(i64_ty, i_ptr, "i").unwrap().into_int_value()], "elem").unwrap()
                        };
                        let val = self.builder.build_load(i64_ty, elem, &name.name).unwrap();
                        let var_ptr = self.create_entry_alloca(i64_ty.into(), &name.name);
                        self.builder.build_store(var_ptr, val).unwrap();
                        self.variables.insert(name.name.clone(), (var_ptr, i64_ty.into()));
                        self.loop_stack.push((inc_bb, after_bb));
                        self.compile_block_stmts(body, false)?;
                        self.loop_stack.pop();
                        if !self.has_terminator() { self.builder.build_unconditional_branch(inc_bb).unwrap(); }
                        self.builder.position_at_end(inc_bb);
                        let i2 = self.builder.build_load(i64_ty, i_ptr, "i").unwrap();
                        let inc = self.builder.build_int_add(i2.into_int_value(), i64_ty.const_int(1, false), "inc").unwrap();
                        self.builder.build_store(i_ptr, inc).unwrap();
                        self.builder.build_unconditional_branch(cond_bb).unwrap();
                        self.builder.position_at_end(after_bb);
                    }
                }
                Ok(Some(i64_ty.const_zero().into()))
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
            Stmt::Break => {
                if let Some((_, after_bb)) = self.loop_stack.last() {
                    self.builder.build_unconditional_branch(*after_bb).unwrap();
                }
                Ok(None)
            }
            Stmt::Continue => {
                if let Some((cond_bb, _)) = self.loop_stack.last() {
                    self.builder.build_unconditional_branch(*cond_bb).unwrap();
                }
                Ok(None)
            }
            Stmt::Expr(expr) => {
                let val = self.compile_expr(expr)?;
                Ok(Some(val))
            }
            Stmt::Destructure { names, value } => {
                let val = self.compile_expr(value)?;
                let ptr = val.into_pointer_value();
                let i64_ty = self.context.i64_type();
                for (i, name) in names.iter().enumerate() {
                    let gep = unsafe { self.builder.build_gep(i64_ty, ptr, &[self.context.i32_type().const_int(i as u64, false)], &name.name).unwrap() };
                    let loaded = self.builder.build_load(i64_ty, gep, &name.name).unwrap();
                    let alloca = self.create_entry_alloca(i64_ty.into(), &name.name);
                    self.builder.build_store(alloca, loaded).unwrap();
                    self.variables.insert(name.name.clone(), (alloca, i64_ty.into()));
                }
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
                if !self.has_terminator() { self.builder.build_unconditional_branch(merge_bb).unwrap(); }
                self.builder.position_at_end(else_bb);
                if let Some(eb) = else_branch { self.compile_block_stmts(eb, false)?; }
                if !self.has_terminator() { self.builder.build_unconditional_branch(merge_bb).unwrap(); }
                self.builder.position_at_end(merge_bb);
                Ok(Some(self.context.i64_type().const_zero().into()))
            }
            Stmt::Try { try_block, catch_var, catch_block } => {
                let i64_ty = self.context.i64_type();
                let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());

                // Allocate caught flag (i64) and error pointer
                let caught_flag = self.create_entry_alloca(i64_ty.into(), "__caught");
                self.builder.build_store(caught_flag, i64_ty.const_zero()).unwrap();
                let err_alloca = self.create_entry_alloca(ptr_ty.into(), "__err");
                self.builder.build_store(err_alloca, ptr_ty.const_null()).unwrap();

                let cf = self.function.unwrap();
                let try_bb = self.context.append_basic_block(cf, "try_body");
                let catch_bb = self.context.append_basic_block(cf, "catch_body");
                let merge_bb = self.context.append_basic_block(cf, "try_merge");
                let after_try_bb = self.context.append_basic_block(cf, "after_try");

                // Store try_end_bb so throw can jump to it
                self.err_ptr = Some(err_alloca);
                self.caught_flag = Some(caught_flag);
                self.try_end_bb = Some(after_try_bb);

                // Enter try block
                self.builder.build_unconditional_branch(try_bb).unwrap();
                self.builder.position_at_end(try_bb);
                self.compile_block_stmts(try_block, false)?;
                if !self.has_terminator() { self.builder.build_unconditional_branch(after_try_bb).unwrap(); }

                // After try: check caught flag
                self.builder.position_at_end(after_try_bb);
                let flag_val = self.builder.build_load(i64_ty, caught_flag, "flag").unwrap().into_int_value();
                let is_caught = self.builder.build_int_compare(inkwell::IntPredicate::NE, flag_val, i64_ty.const_zero(), "is_caught").unwrap();
                self.builder.build_conditional_branch(is_caught, catch_bb, merge_bb).unwrap();

                // Catch block
                self.builder.position_at_end(catch_bb);
                let err_val = self.builder.build_load(ptr_ty, err_alloca, "err_val").unwrap();
                let catch_alloca = self.create_entry_alloca(ptr_ty.into(), &catch_var.name);
                self.builder.build_store(catch_alloca, err_val).unwrap();
                self.variables.insert(catch_var.name.clone(), (catch_alloca, ptr_ty.into()));
                self.compile_block_stmts(catch_block, false)?;
                if !self.has_terminator() { self.builder.build_unconditional_branch(merge_bb).unwrap(); }

                self.builder.position_at_end(merge_bb);
                self.err_ptr = None;
                self.caught_flag = None;
                self.try_end_bb = None;
                Ok(Some(i64_ty.const_zero().into()))
            }
            Stmt::Throw { value } => {
                let i64_ty = self.context.i64_type();
                let err_val = self.compile_expr(value)?;

                if let Some(ep) = self.err_ptr {
                    self.builder.build_store(ep, err_val).unwrap();
                }
                if let Some(cf) = self.caught_flag {
                    self.builder.build_store(cf, i64_ty.const_int(1, false)).unwrap();
                }
                // Jump to after the try block (skip remaining try body)
                if let Some(te) = self.try_end_bb {
                    self.builder.build_unconditional_branch(te).unwrap();
                }

                Ok(Some(i64_ty.const_zero().into()))
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
                self.loop_stack.push((cond_bb, after_bb));
                self.compile_block_stmts(body, false)?;
                self.loop_stack.pop();
                if !self.has_terminator() { self.builder.build_unconditional_branch(cond_bb).unwrap(); }
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
        match entry.get_first_instruction() {
            Some(first_inst) => self.builder.position_before(&first_inst),
            None => self.builder.position_at_end(entry),
        }
        let alloca = self.builder.build_alloca(ty, name).unwrap();
        self.builder.position_at_end(saved);
        alloca
    }

    pub fn has_terminator(&self) -> bool {
        match self.builder.get_insert_block() {
            Some(block) => block.get_terminator().is_some(),
            None => false,
        }
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
            Some(t) => self.type_to_llvm(t).into(),
            None => self.context.i64_type().into(),
        }
    }

    pub fn type_to_llvm(&self, type_: &azurite_parser::ast::Type) -> BasicTypeEnum<'ctx> {
        match type_ {
            azurite_parser::ast::Type::Name(n) if n == "string" => self.context.ptr_type(inkwell::AddressSpace::default()).into(),
            azurite_parser::ast::Type::Name(n) if n == "int" => self.context.i64_type().into(),
            azurite_parser::ast::Type::Name(n) if n == "float" => self.context.f64_type().into(),
            azurite_parser::ast::Type::Name(n) if n == "bool" => self.context.i64_type().into(),
            azurite_parser::ast::Type::Name(n) if self.struct_types.contains_key(n) => {
                self.context.ptr_type(inkwell::AddressSpace::default()).into()
            }
            azurite_parser::ast::Type::Tuple(_) => {
                self.context.ptr_type(inkwell::AddressSpace::default()).into()
            }
            azurite_parser::ast::Type::Name(n) if n == "any" => {
                self.context.i64_type().into()
            }
            _ => self.context.i64_type().into(),
        }
    }

    pub fn field_type_to_llvm(&self, type_: &azurite_parser::ast::Type) -> BasicTypeEnum<'ctx> {
        self.type_to_llvm(type_)
    }
}
