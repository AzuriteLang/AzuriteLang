use azurite_errors::{AzError, ErrorKind};
use azurite_lexer::Span;
use azurite_parser::ast::*;
use inkwell::values::{BasicMetadataValueEnum, BasicValueEnum};
use inkwell::IntPredicate;
use crate::codegen::CodeGen;

pub fn compile_expr<'ctx>(cg: &mut CodeGen<'ctx>, expr: &Expr) -> Result<BasicValueEnum<'ctx>, AzError> {
    match expr {
        Expr::Int(n) => Ok(cg.context.i64_type().const_int(*n as u64, false).into()),
        Expr::Float(n) => Ok(cg.context.f64_type().const_float(*n).into()),
        Expr::String(s) => {
            let ptr = cg.builder.build_global_string_ptr(s, "str").unwrap();
            Ok(ptr.as_pointer_value().into())
        }
        Expr::Bool(b) => Ok(cg.context.i64_type().const_int(*b as u64, false).into()),
        Expr::Null => Ok(cg.context.i64_type().const_zero().into()),
        Expr::Char(c) => Ok(cg.context.i64_type().const_int(*c as u64, false).into()),
        Expr::Self_ | Expr::Super => {
            match cg.self_ptr {
                Some(ptr) => {
                    let loaded = cg.builder.build_load(
                        cg.context.ptr_type(inkwell::AddressSpace::default()),
                        ptr, "self",
                    ).unwrap();
                    Ok(loaded)
                }
                None => Err(AzError::new(ErrorKind::Semantic, Span::new(0, 0, 0, 0), "'self' used outside method")),
            }
        }
        Expr::FieldAccess { obj, field } => {
            super::class::compile_field_access(cg, obj, field)
        }
        Expr::MethodCall { obj, method, args } => {
            super::class::compile_method_call(cg, obj, method, args)
        }
        Expr::Ident(ident) => {
            if let Some((ptr, ty)) = cg.variables.get(&ident.name) {
                let loaded = cg.builder.build_load(*ty, *ptr, &ident.name).unwrap();
                Ok(loaded)
            } else if let Some(f) = cg.module.get_function(&ident.name) {
                let result = cg.builder.build_call(f, &[], "calltmp").unwrap();
                Ok(match result.try_as_basic_value() {
                    inkwell::values::ValueKind::Basic(bv) => bv,
                    _ => cg.context.i64_type().const_zero().into(),
                })
            } else if is_class_name(cg, &ident.name) {
                // Class names used in constructor calls: Person.new(...)
                Ok(cg.context.i64_type().const_zero().into())
            } else {
                Err(AzError::new(ErrorKind::Semantic, ident.span, format!("undefined '{}'", ident.name)))
            }
        }
        Expr::Binary { left, op, right } => {
            if *op == BinOp::Assign {
                return compile_assign(cg, left, right);
            }
            let lhs = cg.compile_expr(left)?;
            let rhs = cg.compile_expr(right)?;
            compile_binary(cg, lhs, rhs, *op)
        }
        Expr::Unary { op, operand } => {
            let val = cg.compile_expr(operand)?;
            match op {
                UnOp::Neg => {
                    let zero = cg.context.i64_type().const_zero();
                    let i = val.into_int_value();
                    Ok(cg.builder.build_int_sub(zero, i, "negtmp").unwrap().into())
                }
                UnOp::Not => {
                    let i = val.into_int_value();
                    Ok(cg.builder.build_not(i, "nottmp").unwrap().into())
                }
            }
        }
        Expr::Call { callee, args } => {
            let callee_name = match callee.as_ref() {
                Expr::Ident(i) => i.name.clone(),
                _ => return Err(AzError::new(ErrorKind::Semantic, Span::new(0, 0, 0, 0), "invalid callee")),
            };

            match callee_name.as_str() {
                "print" => return super::builtin::compile_print(cg, args),
                "sqrt" => return compile_sqrt(cg, args),
                "abs" => return compile_abs(cg, args),
                "len" => return compile_len(cg, args),
                "int" => return compile_int_cast(cg, args),
                "float" => return compile_float_cast(cg, args),
                "read" => return compile_read(cg),
                "input" => return compile_input(cg, args),
                "exit" => return compile_exit(cg, args),
                _ => {}
            }

            let compiled = args.iter()
                .map(|a| cg.compile_expr(a))
                .collect::<Result<Vec<_>, _>>()?;
            let meta: Vec<BasicMetadataValueEnum> = compiled.iter().map(|a| (*a).into()).collect();

            if let Some(f) = cg.module.get_function(&callee_name) {
                let result = cg.builder.build_call(f, &meta, "calltmp").unwrap();
                Ok(match result.try_as_basic_value() {
                    inkwell::values::ValueKind::Basic(bv) => bv,
                    _ => cg.context.i64_type().const_zero().into(),
                })
            } else {
                Err(AzError::new(ErrorKind::Semantic, Span::new(0, 0, 0, 0), format!("undefined '{}'", callee_name)))
            }
        }
        Expr::Block(stmts) => {
            let mut last: Option<BasicValueEnum<'ctx>> = None;
            for stmt in stmts {
                last = cg.compile_stmt(stmt, false)?.or(last);
            }
            last.ok_or_else(|| AzError::new(ErrorKind::Semantic, Span::new(0, 0, 0, 0), "empty block"))
        }
        Expr::If { condition, then_branch, else_branch } => {
            let cond = cg.compile_expr(condition)?;
            let cond_int = cg.to_bool(cond);
            let cf = cg.function.unwrap();
            let then_bb = cg.context.append_basic_block(cf, "then");
            let else_bb = cg.context.append_basic_block(cf, "else");
            let merge_bb = cg.context.append_basic_block(cf, "ifcont");
            cg.builder.build_conditional_branch(cond_int, then_bb, else_bb).unwrap();
            cg.builder.position_at_end(then_bb);
            cg.compile_block_stmts(then_branch, false)?;
            if !cg.has_terminator() { cg.builder.build_unconditional_branch(merge_bb).unwrap(); }
            cg.builder.position_at_end(else_bb);
            if let Some(e) = else_branch { cg.compile_block_stmts(e, false)?; }
            if !cg.has_terminator() { cg.builder.build_unconditional_branch(merge_bb).unwrap(); }
            cg.builder.position_at_end(merge_bb);
            Ok(cg.context.i64_type().const_zero().into())
        }
        Expr::Array(elems) => {
            let count = elems.len() as u32;
            if count == 0 { return Ok(cg.context.i64_type().const_zero().into()); }

            let i64_ty = cg.context.i64_type();
            let size = i64_ty.const_int(count as u64, false);

            let ptr = cg.builder.build_array_malloc(i64_ty, size, "arr").unwrap();
            for (i, elem) in elems.iter().enumerate() {
                let val = cg.compile_expr(elem)?;
                let gep = unsafe {
                    cg.builder.build_gep(i64_ty, ptr, &[cg.context.i32_type().const_int(i as u64, false)], "idx").unwrap()
                };
                cg.builder.build_store(gep, val).unwrap();
            }
            Ok(ptr.into())
        }
        Expr::Index { obj, index } => {
            let obj_val = cg.compile_expr(obj)?;
            let idx_val = cg.compile_expr(index)?;
            let ptr = obj_val.into_pointer_value();
            let idx_int = idx_val.into_int_value();

            let elem = unsafe {
                cg.builder.build_gep(
                    cg.context.i64_type(),
                    ptr,
                    &[idx_int],
                    "elem",
                ).unwrap()
            };
            let loaded = cg.builder.build_load(cg.context.i64_type(), elem, "loaded").unwrap();
            Ok(loaded)
        }
        Expr::EnumVariant { enum_name: _en, variant, .. } => {
            let tag = variant.as_bytes().iter().fold(0u64, |acc, b| acc.wrapping_add(*b as u64));
            let tag_val = cg.context.i8_type().const_int(tag % 256, false);
            Ok(tag_val.into())
        }
        Expr::Match { value, arms } => {
            let val = cg.compile_expr(value)?.into_int_value();
            let cf = cg.function.unwrap();
            let after_bb = cg.context.append_basic_block(cf, "match_after");
            let i64_ty = cg.context.i64_type();
            let start_block = cg.builder.get_insert_block().unwrap();

            for (i, arm) in arms.iter().enumerate() {
                let is_last = i + 1 == arms.len();
                let arm_bb = cg.context.append_basic_block(cf, &format!("match_arm{}", i));

                if !is_last {
                    let rest_bb = cg.context.append_basic_block(cf, &format!("match_rest{}", i));
                    let arm_val = i64_ty.const_int(i as u64, false);
                    if i == 0 {
                        cg.builder.position_at_end(start_block);
                    }
                    let cmp = cg.builder.build_int_compare(
                        inkwell::IntPredicate::EQ, val, arm_val, "matchcmp",
                    ).unwrap();
                    cg.builder.build_conditional_branch(cmp, arm_bb, rest_bb).unwrap();

                    cg.builder.position_at_end(arm_bb);
                    cg.compile_expr(&arm.body)?;
                    if !cg.has_terminator() { cg.builder.build_unconditional_branch(after_bb).unwrap(); }

                    cg.builder.position_at_end(rest_bb);
                } else {
                    // Wildcard: always branch from current block
                    cg.builder.build_unconditional_branch(arm_bb).unwrap();
                    cg.builder.position_at_end(arm_bb);
                    cg.compile_expr(&arm.body)?;
                    if !cg.has_terminator() { cg.builder.build_unconditional_branch(after_bb).unwrap(); }
                }
            }

            cg.builder.position_at_end(after_bb);
            Ok(i64_ty.const_zero().into())
        }
        Expr::Range { start, end } => {
            let _s = cg.compile_expr(start)?;
            cg.compile_expr(end)
        }
        Expr::While { condition, body } => {
            let cf = cg.function.unwrap();
            let cond_bb = cg.context.append_basic_block(cf, "while_cond");
            let body_bb = cg.context.append_basic_block(cf, "while_body");
            let after_bb = cg.context.append_basic_block(cf, "while_after");
            cg.builder.build_unconditional_branch(cond_bb).unwrap();
            cg.builder.position_at_end(cond_bb);
            let cond = cg.compile_expr(condition)?;
            let cond_int = cg.to_bool(cond);
            cg.builder.build_conditional_branch(cond_int, body_bb, after_bb).unwrap();
            cg.builder.position_at_end(body_bb);
            cg.compile_block_stmts(body, false)?;
            if !cg.has_terminator() { cg.builder.build_unconditional_branch(cond_bb).unwrap(); }
            cg.builder.position_at_end(after_bb);
            Ok(cg.context.i64_type().const_zero().into())
        }
    }
}

fn compile_binary<'ctx>(cg: &CodeGen<'ctx>, lhs: BasicValueEnum<'ctx>, rhs: BasicValueEnum<'ctx>, op: BinOp) -> Result<BasicValueEnum<'ctx>, AzError> {
    match (lhs, rhs) {
        (BasicValueEnum::IntValue(l), BasicValueEnum::IntValue(r)) => {
            let i64 = cg.context.i64_type();
            let val = match op {
                BinOp::Add => cg.builder.build_int_add(l, r, "addtmp").unwrap().into(),
                BinOp::Sub => cg.builder.build_int_sub(l, r, "subtmp").unwrap().into(),
                BinOp::Mul => cg.builder.build_int_mul(l, r, "multmp").unwrap().into(),
                BinOp::Div => cg.builder.build_int_signed_div(l, r, "divtmp").unwrap().into(),
                BinOp::Mod => cg.builder.build_int_signed_rem(l, r, "modtmp").unwrap().into(),
                BinOp::Eq | BinOp::Neq | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => {
                    let pred = match op {
                        BinOp::Eq => IntPredicate::EQ, BinOp::Neq => IntPredicate::NE,
                        BinOp::Lt => IntPredicate::SLT, BinOp::Gt => IntPredicate::SGT,
                        BinOp::Le => IntPredicate::SLE, BinOp::Ge => IntPredicate::SGE,
                        _ => unreachable!(),
                    };
                    let cmp = cg.builder.build_int_compare(pred, l, r, "cmptmp").unwrap();
                    cg.builder.build_int_z_extend(cmp, i64, "cmpext").unwrap().into()
                }
                BinOp::And | BinOp::BitAnd => cg.builder.build_and(l, r, "andtmp").unwrap().into(),
                BinOp::Or | BinOp::BitOr => cg.builder.build_or(l, r, "ortmp").unwrap().into(),
                BinOp::BitXor => cg.builder.build_xor(l, r, "xortmp").unwrap().into(),
                BinOp::Shl => cg.builder.build_left_shift(l, r, "shltmp").unwrap().into(),
                BinOp::Shr => cg.builder.build_right_shift(l, r, false, "shrtmp").unwrap().into(),
                BinOp::Assign => return Err(AzError::new(ErrorKind::Semantic, Span::new(0, 0, 0, 0), "assign not in expr")),
            };
            Ok(val)
        }
            (BasicValueEnum::FloatValue(l), BasicValueEnum::FloatValue(r)) => {
                let val = match op {
                    BinOp::Add => cg.builder.build_float_add(l, r, "faddtmp").unwrap().into(),
                    BinOp::Sub => cg.builder.build_float_sub(l, r, "fsubtmp").unwrap().into(),
                    BinOp::Mul => cg.builder.build_float_mul(l, r, "fmultmp").unwrap().into(),
                    BinOp::Div => cg.builder.build_float_div(l, r, "fdivtmp").unwrap().into(),
                    _ => return Err(AzError::new(ErrorKind::Semantic, Span::new(0, 0, 0, 0), "unsupported float op")),
                };
                Ok(val)
            }
            (BasicValueEnum::PointerValue(l), BasicValueEnum::PointerValue(r)) if op == BinOp::Add => {
                // String concatenation: buf = malloc(strlen(l) + strlen(r) + 1); strcpy(buf, l); strcat(buf, r)
                let i64_ty = cg.context.i64_type();
                let ptr_ty = cg.context.ptr_type(inkwell::AddressSpace::default());

                let strlen_ty = i64_ty.fn_type(&[ptr_ty.into()], false);
                cg.module.add_function("strlen", strlen_ty, None);
                let strcpy_ty = ptr_ty.fn_type(&[ptr_ty.into(), ptr_ty.into()], false);
                cg.module.add_function("strcpy", strcpy_ty, None);
                let strcat_ty = ptr_ty.fn_type(&[ptr_ty.into(), ptr_ty.into()], false);
                cg.module.add_function("strcat", strcat_ty, None);

                let strlen = cg.module.get_function("strlen").unwrap();

                let len_l = cg.builder.build_call(strlen, &[l.into()], "llen").unwrap();
                let len_r = cg.builder.build_call(strlen, &[r.into()], "rlen").unwrap();
                let ll = len_l.try_as_basic_value().unwrap_basic().into_int_value();
                let lr = len_r.try_as_basic_value().unwrap_basic().into_int_value();
                let total = cg.builder.build_int_add(ll, lr, "totlen").unwrap();
                let one = i64_ty.const_int(1, false);
                let size = cg.builder.build_int_add(total, one, "mallocsize").unwrap();

                let malloc_ty = ptr_ty.fn_type(&[i64_ty.into()], false);
                cg.module.add_function("malloc", malloc_ty, None);
                let buf = cg.builder.build_call(
                    cg.module.get_function("malloc").unwrap(), &[size.into()], "strbuf",
                ).unwrap().try_as_basic_value().unwrap_basic().into_pointer_value();

                let strcpy_f = cg.module.get_function("strcpy").unwrap();
                cg.builder.build_call(strcpy_f, &[buf.into(), l.into()], "cpy").unwrap();
                let strcat_f = cg.module.get_function("strcat").unwrap();
                cg.builder.build_call(strcat_f, &[buf.into(), r.into()], "cat").unwrap();

                Ok(buf.into())
            }
        _ => Err(AzError::new(ErrorKind::Semantic, Span::new(0, 0, 0, 0), "type mismatch")),
    }
}

fn is_class_name<'ctx>(cg: &CodeGen<'ctx>, name: &str) -> bool {
    cg.struct_types.contains_key(name) || name == "Person" || name == "Option" || name == "Result" || name == "Array"
}

// --- Built-in function implementations ---

fn compile_sqrt<'ctx>(cg: &mut CodeGen<'ctx>, args: &[Expr]) -> Result<BasicValueEnum<'ctx>, AzError> {
    let val = cg.compile_expr(&args[0])?;
    let f = val.into_float_value();

    let name = "llvm.sqrt.f64";
    let f64_ty = cg.context.f64_type();
    let fn_type = f64_ty.fn_type(&[f64_ty.into()], false);

    let intrinsic = cg.module.add_function(name, fn_type, None);
    let result = cg.builder.build_call(intrinsic, &[f.into()], "sqrt").unwrap();
    Ok(match result.try_as_basic_value() {
        inkwell::values::ValueKind::Basic(bv) => bv,
        _ => cg.context.f64_type().const_float(0.0).into(),
    })
}

fn compile_abs<'ctx>(cg: &mut CodeGen<'ctx>, args: &[Expr]) -> Result<BasicValueEnum<'ctx>, AzError> {
    let val = cg.compile_expr(&args[0])?;
    let i = val.into_int_value();
    let zero = cg.context.i64_type().const_zero();
    let neg = cg.builder.build_int_neg(i, "neg").unwrap();
    let cmp = cg.builder.build_int_compare(inkwell::IntPredicate::SLT, i, zero, "iscmp").unwrap();
    let result = cg.builder.build_select(cmp, neg, i, "abs").unwrap();
    Ok(result)
}

fn compile_read<'ctx>(cg: &mut CodeGen<'ctx>) -> Result<BasicValueEnum<'ctx>, AzError> {
    // Use fgets: call fgets(buf, size, stdin)
    let _buf = cg.builder.build_alloca(cg.context.i64_type(), "buf").unwrap();
    let _size = cg.context.i64_type().const_int(256, false);

    let ptr_ty = cg.context.ptr_type(inkwell::AddressSpace::default());
    let fgets_type = ptr_ty.fn_type(&[ptr_ty.into(), cg.context.i64_type().into(), ptr_ty.into()], false);
    cg.module.add_function("fgets", fgets_type, None);

    // For now, return empty string
    let empty = cg.builder.build_global_string_ptr("", "empty").unwrap();
    Ok(empty.as_pointer_value().into())
}

fn compile_input<'ctx>(cg: &mut CodeGen<'ctx>, args: &[Expr]) -> Result<BasicValueEnum<'ctx>, AzError> {
    // Print prompt, then read
    let _ = cg.compile_expr(&args[0])?;
    compile_read(cg)
}

fn compile_exit<'ctx>(cg: &mut CodeGen<'ctx>, args: &[Expr]) -> Result<BasicValueEnum<'ctx>, AzError> {
    let val = cg.compile_expr(&args[0])?;
    let i = val.into_int_value();
    let i32_val = cg.builder.build_int_truncate(i, cg.context.i32_type(), "exitcode").unwrap();

    let exit_type = cg.context.void_type().fn_type(&[cg.context.i32_type().into()], false);
    cg.module.add_function("exit", exit_type, None);
    let exit_fn = cg.module.get_function("exit").unwrap();
    cg.builder.build_call(exit_fn, &[i32_val.into()], "exit").unwrap();

    Ok(cg.context.i64_type().const_zero().into())
}

fn compile_len<'ctx>(cg: &mut CodeGen<'ctx>, args: &[Expr]) -> Result<BasicValueEnum<'ctx>, AzError> {
    let val = cg.compile_expr(&args[0])?;
    let ptr = val.into_pointer_value();

    let ptr_ty = cg.context.ptr_type(inkwell::AddressSpace::default());
    let i64_ty = cg.context.i64_type();
    let strlen_type = i64_ty.fn_type(&[ptr_ty.into()], false);
    cg.module.add_function("strlen", strlen_type, None);

    let len = cg.builder.build_call(
        cg.module.get_function("strlen").unwrap(),
        &[ptr.into()],
        "len",
    ).unwrap();

    Ok(match len.try_as_basic_value() {
        inkwell::values::ValueKind::Basic(bv) => bv,
        _ => cg.context.i64_type().const_zero().into(),
    })
}

fn compile_int_cast<'ctx>(cg: &mut CodeGen<'ctx>, args: &[Expr]) -> Result<BasicValueEnum<'ctx>, AzError> {
    let val = cg.compile_expr(&args[0])?;
    let f = val.into_float_value();
    let result = cg.builder.build_float_to_signed_int(f, cg.context.i64_type(), "f2i").unwrap();
    Ok(result.into())
}

fn compile_float_cast<'ctx>(cg: &mut CodeGen<'ctx>, args: &[Expr]) -> Result<BasicValueEnum<'ctx>, AzError> {
    let val = cg.compile_expr(&args[0])?;
    let i = val.into_int_value();
    let result = cg.builder.build_signed_int_to_float(i, cg.context.f64_type(), "i2f").unwrap();
    Ok(result.into())
}

fn compile_assign<'ctx>(cg: &mut CodeGen<'ctx>, left: &Expr, right: &Expr) -> Result<BasicValueEnum<'ctx>, AzError> {
    let var_name = match left {
        Expr::Ident(i) => i.name.clone(),
        _ => return Err(AzError::new(ErrorKind::Semantic, Span::new(0, 0, 0, 0), "left side of assign must be a variable")),
    };

    let rhs = cg.compile_expr(right)?;

    match cg.variables.get(&var_name) {
        Some((ptr, _ty)) => {
            cg.builder.build_store(*ptr, rhs).unwrap();
            Ok(rhs)
        }
        None => Err(AzError::new(ErrorKind::Semantic, Span::new(0, 0, 0, 0), format!("undefined variable '{}' in assignment", var_name))),
    }
}
