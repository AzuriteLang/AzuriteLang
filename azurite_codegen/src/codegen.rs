use std::collections::HashMap;

use azurite_errors::{AzError, ErrorKind};
use azurite_lexer::Span;
use azurite_parser::ast::*;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Module;
use inkwell::types::BasicTypeEnum;
use inkwell::values::{AnyValue, BasicMetadataValueEnum, BasicValueEnum, IntValue, PointerValue};
use inkwell::IntPredicate;

pub struct CodeGen<'ctx> {
    context: &'ctx Context,
    module: Module<'ctx>,
    builder: Builder<'ctx>,
    variables: HashMap<String, PointerValue<'ctx>>,
    function: Option<inkwell::values::FunctionValue<'ctx>>,
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
            function: None,
        }
    }

    pub fn module(&self) -> &Module<'ctx> {
        &self.module
    }

    pub fn compile_program(&mut self, program: &Program) -> Result<(), AzError> {
        for stmt in &program.statements {
            self.compile_stmt(stmt)?;
        }
        Ok(())
    }

    fn compile_stmt(&mut self, stmt: &Stmt) -> Result<BasicValueEnum<'ctx>, AzError> {
        match stmt {
            Stmt::Let { name, value, .. } => {
                let val = self.compile_expr(value)?;
                let alloca = self.create_entry_alloca(
                    BasicTypeEnum::IntType(self.context.i64_type()),
                    &name.name,
                );
                self.builder.build_store(alloca, val).unwrap();
                self.variables.insert(name.name.clone(), alloca);
                Ok(val)
            }
            Stmt::Func { name, params, body, .. } => {
                let param_types: Vec<inkwell::types::BasicMetadataTypeEnum> = params.iter()
                    .map(|p| self.az_type_to_llvm(&p.type_annotation).into())
                    .collect();

                let fn_type = self.context.i64_type().fn_type(&param_types, false);
                let fn_val = self.module.add_function(&name.name, fn_type, None);

                let entry = self.context.append_basic_block(fn_val, "entry");
                self.builder.position_at_end(entry);
                self.function = Some(fn_val);

                for (i, param) in params.iter().enumerate() {
                    if let Some(param_val) = fn_val.get_nth_param(i as u32) {
                        let ptr = self.create_entry_alloca(
                            BasicTypeEnum::IntType(self.context.i64_type()),
                            &param.name.name,
                        );
                        self.builder.build_store(ptr, param_val).unwrap();
                        self.variables.insert(param.name.name.clone(), ptr);
                    }
                }

                self.compile_expr(body)?;

                if self.builder.get_insert_block().is_some() {
                    let zero = self.context.i64_type().const_zero();
                    self.builder.build_return(Some(&zero)).unwrap();
                }

                self.function = None;
                Ok(self.context.i64_type().const_zero().into())
            }
            Stmt::Return { value } => {
                if let Some(val) = value {
                    let compiled = self.compile_expr(val)?;
                    self.builder.build_return(Some(&compiled)).unwrap();
                } else {
                    self.builder.build_return(None).unwrap();
                }
                Ok(self.context.i64_type().const_zero().into())
            }
            Stmt::Expr(expr) => self.compile_expr(expr),
            Stmt::If { condition, then_branch, else_branch } => {
                let cond = self.compile_expr(condition)?;
                let cond_int = self.to_bool(cond);
                let current_fn = self.function.unwrap();

                let then_bb = self.context.append_basic_block(current_fn, "then");
                let else_bb = self.context.append_basic_block(current_fn, "else");
                let merge_bb = self.context.append_basic_block(current_fn, "ifcont");

                self.builder.build_conditional_branch(cond_int, then_bb, else_bb).unwrap();

                self.builder.position_at_end(then_bb);
                self.compile_expr(then_branch)?;
                self.builder.build_unconditional_branch(merge_bb).unwrap();

                self.builder.position_at_end(else_bb);
                if let Some(else_) = else_branch {
                    self.compile_expr(else_)?;
                }
                self.builder.build_unconditional_branch(merge_bb).unwrap();

                self.builder.position_at_end(merge_bb);
                Ok(self.context.i64_type().const_zero().into())
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
                self.compile_expr(body)?;
                self.builder.build_unconditional_branch(cond_bb).unwrap();

                self.builder.position_at_end(after_bb);
                Ok(self.context.i64_type().const_zero().into())
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
            Expr::Ident(ident) => {
                match self.variables.get(&ident.name) {
                    Some(ptr) => {
                        let val = self.builder.build_load(self.context.i64_type(), *ptr, &ident.name).unwrap();
                        Ok(val)
                    }
                    None => {
                        match self.module.get_function(&ident.name) {
                            Some(_fn) => {
                                let result = self.builder.build_call(_fn, &[], "calltmp").unwrap();
                                Ok(result.as_any_value_enum().try_into().unwrap())
                            }
                            None => Err(AzError::new(
                                ErrorKind::Semantic,
                                ident.span,
                                format!("undefined variable '{}'", ident.name),
                            )),
                        }
                    }
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
                        let int_val = val.into_int_value();
                        Ok(self.builder.build_int_sub(zero, int_val, "negtmp").unwrap().into())
                    }
                    UnOp::Not => {
                        let bool_val = val.into_int_value();
                        Ok(self.builder.build_not(bool_val, "nottmp").unwrap().into())
                    }
                }
            }
            Expr::Call { callee, args } => {
                let callee_name = match callee.as_ref() {
                    Expr::Ident(ident) => ident.name.clone(),
                    _ => return Err(AzError::new(ErrorKind::Semantic, Span::new(0, 0, 0, 0), "invalid callee")),
                };

                let compiled_args: Vec<BasicValueEnum> = args.iter()
                    .map(|a| self.compile_expr(a))
                    .collect::<Result<Vec<_>, _>>()?;

                let metadata_args: Vec<BasicMetadataValueEnum> = compiled_args.iter()
                    .map(|a| (*a).into())
                    .collect();

                match self.module.get_function(&callee_name) {
                    Some(fn_val) => {
                        let result = self.builder.build_call(fn_val, &metadata_args, "calltmp").unwrap();
                        let result_val = result.as_any_value_enum();
                        Ok(result_val.try_into().unwrap_or(self.context.i64_type().const_zero().into()))
                    }
                    None => {
                        if callee_name == "print" || callee_name == "println" {
                            self.compile_print(&compiled_args)?;
                            Ok(self.context.i64_type().const_zero().into())
                        } else {
                            Err(AzError::new(
                                ErrorKind::Semantic,
                                Span::new(0, 0, 0, 0),
                                format!("undefined function '{}'", callee_name),
                            ))
                        }
                    }
                }
            }
            Expr::Block(stmts) => {
                let mut last: Option<BasicValueEnum<'ctx>> = None;
                for stmt in stmts {
                    last = Some(self.compile_stmt(stmt)?);
                }
                last.ok_or_else(|| AzError::new(
                    ErrorKind::Semantic,
                    Span::new(0, 0, 0, 0),
                    "empty block",
                ))
            }
            Expr::If { condition, then_branch, else_branch } => {
                let cond = self.compile_expr(condition)?;
                let cond_int = self.to_bool(cond);
                let current_fn = self.function.unwrap();

                let then_bb = self.context.append_basic_block(current_fn, "then");
                let else_bb = self.context.append_basic_block(current_fn, "else");
                let merge_bb = self.context.append_basic_block(current_fn, "ifcont");

                self.builder.build_conditional_branch(cond_int, then_bb, else_bb).unwrap();

                self.builder.position_at_end(then_bb);
                self.compile_expr(then_branch)?;
                self.builder.build_unconditional_branch(merge_bb).unwrap();

                self.builder.position_at_end(else_bb);
                if let Some(else_) = else_branch {
                    self.compile_expr(else_)?;
                }
                self.builder.build_unconditional_branch(merge_bb).unwrap();

                self.builder.position_at_end(merge_bb);
                Ok(self.context.i64_type().const_zero().into())
            }
            Expr::While { condition, body } => {
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
                self.compile_expr(body)?;
                self.builder.build_unconditional_branch(cond_bb).unwrap();

                self.builder.position_at_end(after_bb);
                Ok(self.context.i64_type().const_zero().into())
            }
        }
    }

    fn compile_binary(&self, lhs: BasicValueEnum<'ctx>, rhs: BasicValueEnum<'ctx>, op: BinOp) -> Result<BasicValueEnum<'ctx>, AzError> {
        match (lhs, rhs) {
            (BasicValueEnum::IntValue(l), BasicValueEnum::IntValue(r)) => {
                let i64_ty = self.context.i64_type();
                let val: BasicValueEnum = match op {
                    BinOp::Add => self.builder.build_int_add(l, r, "addtmp").unwrap().into(),
                    BinOp::Sub => self.builder.build_int_sub(l, r, "subtmp").unwrap().into(),
                    BinOp::Mul => self.builder.build_int_mul(l, r, "multmp").unwrap().into(),
                    BinOp::Div => self.builder.build_int_signed_div(l, r, "divtmp").unwrap().into(),
                    BinOp::Mod => self.builder.build_int_signed_rem(l, r, "modtmp").unwrap().into(),
                    BinOp::Eq => {
                        let cmp = self.builder.build_int_compare(IntPredicate::EQ, l, r, "eqtmp").unwrap();
                        self.builder.build_int_z_extend(cmp, i64_ty, "eqext").unwrap().into()
                    }
                    BinOp::Neq => {
                        let cmp = self.builder.build_int_compare(IntPredicate::NE, l, r, "neqtmp").unwrap();
                        self.builder.build_int_z_extend(cmp, i64_ty, "neqext").unwrap().into()
                    }
                    BinOp::Lt => {
                        let cmp = self.builder.build_int_compare(IntPredicate::SLT, l, r, "lttmp").unwrap();
                        self.builder.build_int_z_extend(cmp, i64_ty, "ltext").unwrap().into()
                    }
                    BinOp::Gt => {
                        let cmp = self.builder.build_int_compare(IntPredicate::SGT, l, r, "gttmp").unwrap();
                        self.builder.build_int_z_extend(cmp, i64_ty, "gtext").unwrap().into()
                    }
                    BinOp::Le => {
                        let cmp = self.builder.build_int_compare(IntPredicate::SLE, l, r, "letmp").unwrap();
                        self.builder.build_int_z_extend(cmp, i64_ty, "leext").unwrap().into()
                    }
                    BinOp::Ge => {
                        let cmp = self.builder.build_int_compare(IntPredicate::SGE, l, r, "getmp").unwrap();
                        self.builder.build_int_z_extend(cmp, i64_ty, "geext").unwrap().into()
                    }
                    BinOp::And => self.builder.build_and(l, r, "andtmp").unwrap().into(),
                    BinOp::Or => self.builder.build_or(l, r, "ortmp").unwrap().into(),
                    BinOp::BitAnd => self.builder.build_and(l, r, "bandtmp").unwrap().into(),
                    BinOp::BitOr => self.builder.build_or(l, r, "bortmp").unwrap().into(),
                    BinOp::BitXor => self.builder.build_xor(l, r, "xortmp").unwrap().into(),
                    BinOp::Shl => self.builder.build_left_shift(l, r, "shltmp").unwrap().into(),
                    BinOp::Shr => self.builder.build_right_shift(l, r, false, "shrtmp").unwrap().into(),
                    BinOp::Assign => return Err(AzError::new(ErrorKind::Semantic, Span::new(0, 0, 0, 0), "assign handled at stmt level")),
                };
                Ok(val)
            }
            (BasicValueEnum::FloatValue(l), BasicValueEnum::FloatValue(r)) => {
                let val: BasicValueEnum = match op {
                    BinOp::Add => self.builder.build_float_add(l, r, "faddtmp").unwrap().into(),
                    BinOp::Sub => self.builder.build_float_sub(l, r, "fsubtmp").unwrap().into(),
                    BinOp::Mul => self.builder.build_float_mul(l, r, "fmultmp").unwrap().into(),
                    BinOp::Div => self.builder.build_float_div(l, r, "fdivtmp").unwrap().into(),
                    _ => return Err(AzError::new(ErrorKind::Semantic, Span::new(0, 0, 0, 0), "unsupported float op")),
                };
                Ok(val)
            }
            _ => Err(AzError::new(ErrorKind::Semantic, Span::new(0, 0, 0, 0), "type mismatch in binary op")),
        }
    }

    fn compile_print(&self, _args: &[BasicValueEnum<'ctx>]) -> Result<(), AzError> {
        let printf_type = self.context.i64_type().fn_type(
            &[self.context.ptr_type(inkwell::AddressSpace::default()).into()],
            true,
        );
        self.module.add_function("printf", printf_type, None);
        Ok(())
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
                let cmp = self.builder.build_float_compare(
                    inkwell::FloatPredicate::ONE, f, zero, "booltmp",
                ).unwrap();
                self.builder.build_int_z_extend(cmp, self.context.i64_type(), "booltmp").unwrap()
            }
            v => {
                let i: IntValue = v.into_int_value();
                i
            }
        }
    }

    fn az_type_to_llvm(&self, type_: &Option<azurite_parser::ast::Type>) -> BasicTypeEnum<'ctx> {
        match type_ {
            Some(azurite_parser::ast::Type::Name(n)) if n == "int" => self.context.i64_type().into(),
            Some(azurite_parser::ast::Type::Name(n)) if n == "float" => self.context.f64_type().into(),
            Some(azurite_parser::ast::Type::Name(n)) if n == "bool" => self.context.bool_type().into(),
            _ => self.context.i64_type().into(),
        }
    }
}
