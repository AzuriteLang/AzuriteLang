use std::collections::HashMap;

use azurite_errors::{AzError, ErrorKind};
use azurite_lexer::Span;
use azurite_parser::ast::*;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::types::{BasicMetadataTypeEnum, BasicTypeEnum};
use inkwell::values::{BasicMetadataValueEnum, BasicValueEnum, FunctionValue, IntValue, PointerValue};
use inkwell::IntPredicate;

pub struct CodeGen<'ctx> {
    context: &'ctx Context,
    module: Module<'ctx>,
    builder: Builder<'ctx>,
    variables: HashMap<String, (PointerValue<'ctx>, BasicTypeEnum<'ctx>)>,
    struct_types: HashMap<String, (Vec<BasicTypeEnum<'ctx>>, inkwell::types::StructType<'ctx>)>,
    function: Option<FunctionValue<'ctx>>,
    self_ptr: Option<PointerValue<'ctx>>,
    printf: Option<FunctionValue<'ctx>>,
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

    fn compile_stmt(&mut self, stmt: &Stmt, _is_tail: bool) -> Result<Option<BasicValueEnum<'ctx>>, AzError> {
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
                let is_void = return_type.is_none() || matches!(return_type, Some(azurite_parser::ast::Type::Name(n)) if n == "void");

                let param_types: Vec<BasicMetadataTypeEnum> = params.iter()
                    .map(|p| self.az_param_type(&p.type_annotation))
                    .collect();

                let fn_type = if is_void {
                    self.context.void_type().fn_type(&param_types, false)
                } else {
                    self.context.i64_type().fn_type(&param_types, false)
                };
                let fn_val = self.module.add_function(&name.name, fn_type, None);
                let entry = self.context.append_basic_block(fn_val, "entry");
                self.builder.position_at_end(entry);
                self.function = Some(fn_val);

                for (i, param) in params.iter().enumerate() {
                    if let Some(param_val) = fn_val.get_nth_param(i as u32) {
                        let ptr = self.create_entry_alloca(param_val.get_type(), &param.name.name);
                        self.builder.build_store(ptr, param_val).unwrap();
                        self.variables.insert(param.name.name.clone(), (ptr, param_val.get_type()));
                    }
                }

                let last_val = self.compile_block_stmts(body, true)?;
                let has_terminator = self.builder.get_insert_block()
                    .and_then(|b| b.get_last_instruction())
                    .is_some();

                if !has_terminator {
                    if is_void {
                        self.builder.build_return(None).unwrap();
                    } else if let Some(val) = last_val {
                        let int_val = self.any_to_i64(val);
                        self.builder.build_return(Some(&int_val)).unwrap();
                    } else {
                        let zero = self.context.i64_type().const_zero();
                        self.builder.build_return(Some(&zero)).unwrap();
                    }
                }

                self.function = None;
                self.self_ptr = None;
                Ok(None)
            }
            Stmt::Class { name, fields, methods } => {
                let member_types: Vec<BasicTypeEnum> = fields.iter()
                    .map(|f| match &f.type_ {
                        azurite_parser::ast::Type::Name(n) if n == "string" => self.context.ptr_type(inkwell::AddressSpace::default()).into(),
                        azurite_parser::ast::Type::Name(n) if n == "int" => self.context.i64_type().into(),
                        azurite_parser::ast::Type::Name(n) if n == "float" => self.context.f64_type().into(),
                        azurite_parser::ast::Type::Name(n) if n == "bool" => self.context.i64_type().into(),
                        _ => self.context.i64_type().into(),
                    })
                    .collect();

                let struct_name = format!("struct.{}", name.name);
                let llvm_struct = self.context.opaque_struct_type(&struct_name);
                llvm_struct.set_body(&member_types, false);
                self.struct_types.insert(name.name.clone(), (member_types, llvm_struct));

                for method in methods {
                    if let Stmt::Func { name: mname, params, return_type, body } = method {
                        let self_type = self.context.ptr_type(inkwell::AddressSpace::default());
                        let mut param_types: Vec<BasicMetadataTypeEnum> = vec![self_type.into()];
                        for p in params {
                            param_types.push(match &p.type_annotation {
                                Some(azurite_parser::ast::Type::Name(n)) if n == "string" => self.context.ptr_type(inkwell::AddressSpace::default()).into(),
                                Some(azurite_parser::ast::Type::Name(n)) if n == "int" => self.context.i64_type().into(),
                                Some(azurite_parser::ast::Type::Name(n)) if n == "float" => self.context.f64_type().into(),
                                Some(azurite_parser::ast::Type::Name(n)) if n == "bool" => self.context.i64_type().into(),
                                _ => self.context.i64_type().into(),
                            });
                        }

                        let is_void = return_type.is_none() || matches!(return_type, Some(azurite_parser::ast::Type::Name(ref n)) if n == "void" || n == "none");
                        let fn_name = format!("{}_{}", name.name, mname.name);

                        let fn_val = if is_void {
                            let fn_type = self.context.void_type().fn_type(&param_types, false);
                            self.module.add_function(&fn_name, fn_type, None)
                        } else {
                            let fn_type = self.context.i64_type().fn_type(&param_types, false);
                            self.module.add_function(&fn_name, fn_type, None)
                        };

                        let entry = self.context.append_basic_block(fn_val, "entry");
                        self.builder.position_at_end(entry);
                        self.function = Some(fn_val);

                        if let Some(self_param) = fn_val.get_nth_param(0) {
                            let self_alloca = self.create_entry_alloca(self_type.into(), "self");
                            self.builder.build_store(self_alloca, self_param).unwrap();
                            self.self_ptr = Some(self_alloca);
                        }

                        for (i, param) in params.iter().enumerate() {
                            if let Some(pval) = fn_val.get_nth_param((i + 1) as u32) {
                                let ptr = self.create_entry_alloca(pval.get_type(), &param.name.name);
                                self.builder.build_store(ptr, pval).unwrap();
                                self.variables.insert(param.name.name.clone(), (ptr, pval.get_type()));
                            }
                        }

                        self.compile_block_stmts(body, true)?;
                        let has_term = self.builder.get_insert_block()
                            .and_then(|b| b.get_last_instruction()).is_some();
                        if !has_term {
                            if is_void {
                                self.builder.build_return(None).unwrap();
                            } else {
                                self.builder.build_return(Some(&self.context.i64_type().const_zero())).unwrap();
                            }
                        }

                        self.function = None;
                        self.self_ptr = None;
                    }
                }
                Ok(None)
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
                let current_fn = self.function.unwrap();

                let then_bb = self.context.append_basic_block(current_fn, "then");
                let else_bb = self.context.append_basic_block(current_fn, "else");
                let merge_bb = self.context.append_basic_block(current_fn, "ifcont");

                self.builder.build_conditional_branch(cond_int, then_bb, else_bb).unwrap();

                self.builder.position_at_end(then_bb);
                self.compile_block_stmts(then_branch, false)?;
                self.builder.build_unconditional_branch(merge_bb).unwrap();

                self.builder.position_at_end(else_bb);
                if let Some(else_) = else_branch {
                    self.compile_block_stmts(else_, false)?;
                }
                self.builder.build_unconditional_branch(merge_bb).unwrap();

                self.builder.position_at_end(merge_bb);
                Ok(Some(self.context.i64_type().const_zero().into()))
            }
            Stmt::While { condition, body } => {
                let current_fn = self.function.unwrap();

                let cond_bb = self.context.append_basic_block(current_fn, "while_cond");
                let body_bb = self.context.append_basic_block(current_fn, "while_body");
                let after_bb = self.context.append_basic_block(current_fn, "while_after");

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

    fn compile_block_stmts(&mut self, expr: &Expr, tail: bool) -> Result<Option<BasicValueEnum<'ctx>>, AzError> {
        match expr {
            Expr::Block(stmts) => {
                let mut last = None;
                for stmt in stmts {
                    last = self.compile_stmt(stmt, false)?.or(last);
                }
                Ok(last)
            }
            other => {
                if tail {
                    let val = self.compile_expr(other)?;
                    Ok(Some(val))
                } else {
                    self.compile_expr(other)?;
                    Ok(None)
                }
            }
        }
    }

    fn compile_expr(&mut self, expr: &Expr) -> Result<BasicValueEnum<'ctx>, AzError> {
        match expr {
            Expr::Int(n) => Ok(self.context.i64_type().const_int(*n as u64, false).into()),
            Expr::Float(n) => Ok(self.context.f64_type().const_float(*n).into()),
            Expr::String(s) => {
                let ptr = self.builder.build_global_string_ptr(s, "str").unwrap();
                Ok(ptr.as_pointer_value().into())
            }
            Expr::Bool(b) => Ok(self.context.bool_type().const_int(*b as u64, false).into()),
            Expr::Null => Ok(self.context.i64_type().const_zero().into()),
            Expr::Char(c) => Ok(self.context.i64_type().const_int(*c as u64, false).into()),
            Expr::Self_ => {
                match self.self_ptr {
                    Some(ptr) => {
                        let loaded = self.builder.build_load(
                            self.context.ptr_type(inkwell::AddressSpace::default()),
                            ptr, "self",
                        ).unwrap();
                        Ok(loaded)
                    }
                    None => Err(AzError::new(ErrorKind::Semantic, Span::new(0, 0, 0, 0), "'self' used outside method")),
                }
            }
            Expr::FieldAccess { obj, .. } => {
                let _ = self.compile_expr(obj)?;
                Ok(self.context.i64_type().const_zero().into())
            }
            Expr::MethodCall { obj, method, args } => {
                let _obj_val = self.compile_expr(obj)?;
                let compiled_args = args.iter()
                    .map(|a| self.compile_expr(a))
                    .collect::<Result<Vec<_>, _>>()?;

                // Method calls need class name context; simplified for now
                let _meta_args: Vec<BasicMetadataValueEnum> = compiled_args.iter()
                    .map(|a| (*a).into())
                    .collect();
                Ok(self.context.i64_type().const_zero().into())
            }
            Expr::Ident(ident) => {
                if let Some((ptr, ty)) = self.variables.get(&ident.name) {
                    let loaded = self.builder.build_load(*ty, *ptr, &ident.name).unwrap();
                    Ok(loaded)
                } else if let Some(f) = self.module.get_function(&ident.name) {
                    let result = self.builder.build_call(f, &[], "calltmp").unwrap();
                    Ok(match result.try_as_basic_value() {
                        inkwell::values::ValueKind::Basic(bv) => bv,
                        _ => self.context.i64_type().const_zero().into(),
                    })
                } else {
                    Err(AzError::new(ErrorKind::Semantic, ident.span, format!("undefined '{}'", ident.name)))
                }
            }
            Expr::Binary { left, op, right } => {
                let lhs = self.compile_expr(left)?;
                let rhs = self.compile_expr(right)?;
                self.compile_binary(lhs, rhs, *op)
            }
            Expr::Unary { op, operand } => {
                let val = self.compile_expr(operand)?;
                match op {
                    UnOp::Neg => {
                        let zero = self.context.i64_type().const_zero();
                        let i = val.into_int_value();
                        Ok(self.builder.build_int_sub(zero, i, "negtmp").unwrap().into())
                    }
                    UnOp::Not => {
                        let i = val.into_int_value();
                        Ok(self.builder.build_not(i, "nottmp").unwrap().into())
                    }
                }
            }
            Expr::Call { callee, args } => {
                let callee_name = match callee.as_ref() {
                    Expr::Ident(i) => i.name.clone(),
                    _ => return Err(AzError::new(ErrorKind::Semantic, Span::new(0, 0, 0, 0), "invalid callee")),
                };

                if callee_name == "print" || callee_name == "println" {
                    return self.compile_print(&callee_name, args);
                }

                let compiled_args = args.iter()
                    .map(|a| self.compile_expr(a))
                    .collect::<Result<Vec<_>, _>>()?;

                let metadata_args: Vec<BasicMetadataValueEnum> = compiled_args.iter()
                    .map(|a| (*a).into())
                    .collect();

                if let Some(f) = self.module.get_function(&callee_name) {
                    let result = self.builder.build_call(f, &metadata_args, "calltmp").unwrap();
                    Ok(match result.try_as_basic_value() {
                        inkwell::values::ValueKind::Basic(bv) => bv,
                        _ => self.context.i64_type().const_zero().into(),
                    })
                } else {
                    Err(AzError::new(ErrorKind::Semantic, Span::new(0, 0, 0, 0), format!("undefined '{}'", callee_name)))
                }
            }
            Expr::Block(stmts) => {
                let mut last: Option<BasicValueEnum<'ctx>> = None;
                for stmt in stmts {
                    last = self.compile_stmt(stmt, false)?.or(last);
                }
                last.ok_or_else(|| AzError::new(ErrorKind::Semantic, Span::new(0, 0, 0, 0), "empty block"))
            }
            Expr::If { condition, then_branch, else_branch } => {
                self.compile_if(condition, then_branch, else_branch.as_deref())
            }
            Expr::While { condition, body } => {
                self.compile_while(condition, body)
            }
        }
    }

    fn compile_binary(&self, lhs: BasicValueEnum<'ctx>, rhs: BasicValueEnum<'ctx>, op: BinOp) -> Result<BasicValueEnum<'ctx>, AzError> {
        match (lhs, rhs) {
            (BasicValueEnum::IntValue(l), BasicValueEnum::IntValue(r)) => {
                let i64_ty = self.context.i64_type();
                let val = match op {
                    BinOp::Add => self.builder.build_int_add(l, r, "addtmp").unwrap().into(),
                    BinOp::Sub => self.builder.build_int_sub(l, r, "subtmp").unwrap().into(),
                    BinOp::Mul => self.builder.build_int_mul(l, r, "multmp").unwrap().into(),
                    BinOp::Div => self.builder.build_int_signed_div(l, r, "divtmp").unwrap().into(),
                    BinOp::Mod => self.builder.build_int_signed_rem(l, r, "modtmp").unwrap().into(),
                    BinOp::Eq | BinOp::Neq | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => {
                        let pred = match op {
                            BinOp::Eq => IntPredicate::EQ,
                            BinOp::Neq => IntPredicate::NE,
                            BinOp::Lt => IntPredicate::SLT,
                            BinOp::Gt => IntPredicate::SGT,
                            BinOp::Le => IntPredicate::SLE,
                            BinOp::Ge => IntPredicate::SGE,
                            _ => unreachable!(),
                        };
                        let cmp = self.builder.build_int_compare(pred, l, r, "cmptmp").unwrap();
                        self.builder.build_int_z_extend(cmp, i64_ty, "cmpext").unwrap().into()
                    }
                    BinOp::And | BinOp::BitAnd => self.builder.build_and(l, r, "andtmp").unwrap().into(),
                    BinOp::Or | BinOp::BitOr => self.builder.build_or(l, r, "ortmp").unwrap().into(),
                    BinOp::BitXor => self.builder.build_xor(l, r, "xortmp").unwrap().into(),
                    BinOp::Shl => self.builder.build_left_shift(l, r, "shltmp").unwrap().into(),
                    BinOp::Shr => self.builder.build_right_shift(l, r, false, "shrtmp").unwrap().into(),
                    BinOp::Assign => return Err(AzError::new(ErrorKind::Semantic, Span::new(0, 0, 0, 0), "assign not handled in expr")),
                };
                Ok(val)
            }
            (BasicValueEnum::FloatValue(l), BasicValueEnum::FloatValue(r)) => {
                let val = match op {
                    BinOp::Add => self.builder.build_float_add(l, r, "faddtmp").unwrap().into(),
                    BinOp::Sub => self.builder.build_float_sub(l, r, "fsubtmp").unwrap().into(),
                    BinOp::Mul => self.builder.build_float_mul(l, r, "fmultmp").unwrap().into(),
                    BinOp::Div => self.builder.build_float_div(l, r, "fdivtmp").unwrap().into(),
                    _ => return Err(AzError::new(ErrorKind::Semantic, Span::new(0, 0, 0, 0), "unsupported float op")),
                };
                Ok(val)
            }
            _ => Err(AzError::new(ErrorKind::Semantic, Span::new(0, 0, 0, 0), "type mismatch")),
        }
    }

    fn compile_print(&mut self, name: &str, args: &[Expr]) -> Result<BasicValueEnum<'ctx>, AzError> {
        let add_nl = name == "println";

        let val = if args.is_empty() {
            self.context.i64_type().const_zero().into()
        } else {
            self.compile_expr(&args[0])?
        };

        let (fmt, arg) = self.get_print_format(&val);

        let mut printf_args: Vec<BasicMetadataValueEnum> = vec![fmt.into()];
        if let Some(a) = arg {
            printf_args.push(a.into());
        }

        let printf = self.get_or_declare_printf();
        self.builder.build_call(printf, &printf_args, "printtmp").unwrap();

        if add_nl {
            let nl = self.context.i8_type().const_int(b'\n' as u64, false);
            let putchar = self.get_or_declare_putchar();
            self.builder.build_call(putchar, &[nl.into()], "nl").unwrap();
        }

        Ok(self.context.i64_type().const_zero().into())
    }

    fn get_print_format(&self, val: &BasicValueEnum<'ctx>) -> (PointerValue<'ctx>, Option<BasicValueEnum<'ctx>>) {
        match val {
            BasicValueEnum::IntValue(_) => {
                let global = self.builder.build_global_string_ptr("%d", "intfmt").unwrap();
                (global.as_pointer_value(), Some(*val))
            }
            BasicValueEnum::FloatValue(_) => {
                let global = self.builder.build_global_string_ptr("%g", "floatfmt").unwrap();
                (global.as_pointer_value(), Some(*val))
            }
            BasicValueEnum::PointerValue(_p) => {
                let global = self.builder.build_global_string_ptr("%s", "strfmt").unwrap();
                (global.as_pointer_value(), Some(*val))
            }
            _ => {
                let global = self.builder.build_global_string_ptr("%ld", "defaultfmt").unwrap();
                (global.as_pointer_value(), Some(*val))
            }
        }
    }

    fn get_or_declare_printf(&mut self) -> FunctionValue<'ctx> {
        if let Some(pf) = self.printf { return pf; }
        let i64_ty = self.context.i64_type();
        let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());
        let fn_type = i64_ty.fn_type(&[ptr_ty.into()], true);
        let pf = self.module.add_function("printf", fn_type, None);
        self.printf = Some(pf);
        pf
    }

    fn get_or_declare_putchar(&mut self) -> FunctionValue<'ctx> {
        let i32_ty = self.context.i32_type();
        let fn_type = i32_ty.fn_type(&[i32_ty.into()], false);
        self.module.add_function("putchar", fn_type, None)
    }

    fn compile_if(&mut self, condition: &Expr, then_branch: &Expr, else_branch: Option<&Expr>) -> Result<BasicValueEnum<'ctx>, AzError> {
        let cond = self.compile_expr(condition)?;
        let cond_int = self.to_bool(cond);
        let current_fn = self.function.unwrap();

        let then_bb = self.context.append_basic_block(current_fn, "then");
        let else_bb = self.context.append_basic_block(current_fn, "else");
        let merge_bb = self.context.append_basic_block(current_fn, "ifcont");

        self.builder.build_conditional_branch(cond_int, then_bb, else_bb).unwrap();
        self.builder.position_at_end(then_bb);
        self.compile_block_stmts(then_branch, false)?;
        self.builder.build_unconditional_branch(merge_bb).unwrap();

        self.builder.position_at_end(else_bb);
        if let Some(e) = else_branch {
            self.compile_block_stmts(e, false)?;
        }
        self.builder.build_unconditional_branch(merge_bb).unwrap();

        self.builder.position_at_end(merge_bb);
        Ok(self.context.i64_type().const_zero().into())
    }

    fn compile_while(&mut self, condition: &Expr, body: &Expr) -> Result<BasicValueEnum<'ctx>, AzError> {
        let current_fn = self.function.unwrap();

        let cond_bb = self.context.append_basic_block(current_fn, "while_cond");
        let body_bb = self.context.append_basic_block(current_fn, "while_body");
        let after_bb = self.context.append_basic_block(current_fn, "while_after");

        self.builder.build_unconditional_branch(cond_bb).unwrap();
        self.builder.position_at_end(cond_bb);

        let cond = self.compile_expr(condition)?;
        let cond_int = self.to_bool(cond);
        self.builder.build_conditional_branch(cond_int, body_bb, after_bb).unwrap();

        self.builder.position_at_end(body_bb);
        self.compile_block_stmts(body, false)?;
        self.builder.build_unconditional_branch(cond_bb).unwrap();

        self.builder.position_at_end(after_bb);
        Ok(self.context.i64_type().const_zero().into())
    }

    fn any_to_i64(&self, val: BasicValueEnum<'ctx>) -> BasicValueEnum<'ctx> {
        match val {
            BasicValueEnum::IntValue(i) => {
                let i64_ty = self.context.i64_type();
                if i.get_type() == i64_ty { val }
                else { self.builder.build_int_z_extend(i, i64_ty, "zext").unwrap().into() }
            }
            BasicValueEnum::FloatValue(f) => {
                self.builder.build_float_to_signed_int(f, self.context.i64_type(), "f2i").unwrap().into()
            }
            BasicValueEnum::PointerValue(_p) => self.context.i64_type().const_zero().into(),
            _ => self.context.i64_type().const_zero().into(),
        }
    }

    fn create_entry_alloca(&self, ty: BasicTypeEnum<'ctx>, name: &str) -> PointerValue<'ctx> {
        let entry_bb = self.function.unwrap().get_first_basic_block().unwrap();
        let saved_pos = self.builder.get_insert_block().unwrap();
        self.builder.position_at_end(entry_bb);
        let alloca = self.builder.build_alloca(ty, name).unwrap();
        self.builder.position_at_end(saved_pos);
        alloca
    }

    fn to_bool(&self, val: BasicValueEnum<'ctx>) -> IntValue<'ctx> {
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

    fn az_param_type(&self, type_: &Option<azurite_parser::ast::Type>) -> BasicMetadataTypeEnum<'ctx> {
        match type_ {
            Some(azurite_parser::ast::Type::Name(n)) if n == "string" => self.context.ptr_type(inkwell::AddressSpace::default()).into(),
            Some(azurite_parser::ast::Type::Name(n)) if n == "int" => self.context.i64_type().into(),
            Some(azurite_parser::ast::Type::Name(n)) if n == "float" => self.context.f64_type().into(),
            Some(azurite_parser::ast::Type::Name(n)) if n == "bool" => self.context.i64_type().into(),
            _ => self.context.i64_type().into(),
        }
    }
}
