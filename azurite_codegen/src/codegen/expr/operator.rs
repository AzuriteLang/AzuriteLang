use azurite_errors::{AzError, ErrorKind};
use azurite_lexer::Span;
use azurite_parser::ast::*;
use inkwell::values::BasicValueEnum;
use inkwell::IntPredicate;
use crate::codegen::CodeGen;

pub fn compile_operator<'ctx>(cg: &mut CodeGen<'ctx>, expr: &Expr) -> Result<BasicValueEnum<'ctx>, AzError> {
    match expr {
        Expr::Binary { left, op, right } => {
            let span = expr.span();
            if *op == BinOp::Assign { return compile_assign(cg, left, right, span); }
            if *op == BinOp::Is {
                // The checker already verified the type match. Emit 1 at compile time.
                return Ok(cg.context.i64_type().const_int(1, false).into());
            }
            let lhs = cg.compile_expr(left)?;
            let rhs = cg.compile_expr(right)?;
            compile_binary(cg, lhs, rhs, *op, span)
        }
        Expr::Unary { op, operand } => {
            let span = expr.span();
            let val = cg.compile_expr(operand)?;
            match op {
                UnOp::Neg => {
                    match val {
                        BasicValueEnum::IntValue(i) => {
                            let zero = cg.context.i64_type().const_zero();
                            Ok(cg.builder.build_int_sub(zero, i, "negtmp").unwrap().into())
                        }
                        BasicValueEnum::FloatValue(f) => {
                            Ok(cg.builder.build_float_neg(f, "negftmp").unwrap().into())
                        }
                        _ => return Err(AzError::new(ErrorKind::Semantic, span, "cannot negate this type")),
                    }
                }
                UnOp::Not => {
                    let i = val.into_int_value();
                    let zero = cg.context.i64_type().const_zero();
                    let cmp = cg.builder.build_int_compare(inkwell::IntPredicate::EQ, i, zero, "nottmp").unwrap();
                    let ext = cg.builder.build_int_z_extend(cmp, cg.context.i64_type(), "not_ext").unwrap();
                    Ok(ext.into())
                }
            }
        }
        _ => unreachable!(),
    }
}

fn compile_binary<'ctx>(cg: &CodeGen<'ctx>, lhs: BasicValueEnum<'ctx>, rhs: BasicValueEnum<'ctx>, op: BinOp, span: Span) -> Result<BasicValueEnum<'ctx>, AzError> {
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
                    let pred = match op { BinOp::Eq => IntPredicate::EQ, BinOp::Neq => IntPredicate::NE, BinOp::Lt => IntPredicate::SLT, BinOp::Gt => IntPredicate::SGT, BinOp::Le => IntPredicate::SLE, BinOp::Ge => IntPredicate::SGE, _ => unreachable!() };
                    let cmp = cg.builder.build_int_compare(pred, l, r, "cmptmp").unwrap();
                    cg.builder.build_int_z_extend(cmp, i64, "cmpext").unwrap().into()
                }
                BinOp::And | BinOp::BitAnd => cg.builder.build_and(l, r, "andtmp").unwrap().into(),
                BinOp::Or | BinOp::BitOr => cg.builder.build_or(l, r, "ortmp").unwrap().into(),
                BinOp::BitXor => cg.builder.build_xor(l, r, "xortmp").unwrap().into(),
                BinOp::Shl => cg.builder.build_left_shift(l, r, "shltmp").unwrap().into(),
                BinOp::Shr => cg.builder.build_right_shift(l, r, false, "shrtmp").unwrap().into(),
                BinOp::Assign | BinOp::Is => unreachable!(),
            };
            Ok(val)
        }
        (BasicValueEnum::FloatValue(l), BasicValueEnum::FloatValue(r)) => {
            let i64_ty = cg.context.i64_type();
            let val = match op {
                BinOp::Add => cg.builder.build_float_add(l, r, "faddtmp").unwrap().into(),
                BinOp::Sub => cg.builder.build_float_sub(l, r, "fsubtmp").unwrap().into(),
                BinOp::Mul => cg.builder.build_float_mul(l, r, "fmultmp").unwrap().into(),
                BinOp::Div => cg.builder.build_float_div(l, r, "fdivtmp").unwrap().into(),
                BinOp::Eq | BinOp::Neq | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => {
                    let pred = match op {
                        BinOp::Eq => inkwell::FloatPredicate::OEQ,
                        BinOp::Neq => inkwell::FloatPredicate::ONE,
                        BinOp::Lt => inkwell::FloatPredicate::OLT,
                        BinOp::Gt => inkwell::FloatPredicate::OGT,
                        BinOp::Le => inkwell::FloatPredicate::OLE,
                        BinOp::Ge => inkwell::FloatPredicate::OGE,
                        _ => unreachable!(),
                    };
                    let cmp = cg.builder.build_float_compare(pred, l, r, "fcmptmp").unwrap();
                    cg.builder.build_int_z_extend(cmp, i64_ty, "fcmpext").unwrap().into()
                }
                BinOp::Is => { return Ok(cg.context.i64_type().const_int(1, false).into()); }
                _ => return Err(AzError::new(ErrorKind::Semantic, span, "unsupported float op")),
            };
            Ok(val)
        }
        (BasicValueEnum::PointerValue(l), BasicValueEnum::PointerValue(r)) if op == BinOp::Add => {
            // String concatenation
            compile_string_concat(cg, l, r)
        }
        _ => Err(AzError::new(ErrorKind::Semantic, span, "type mismatch")),
    }
}

fn compile_string_concat<'ctx>(cg: &CodeGen<'ctx>, l: inkwell::values::PointerValue<'ctx>, r: inkwell::values::PointerValue<'ctx>) -> Result<BasicValueEnum<'ctx>, AzError> {
    let i64_ty = cg.context.i64_type();
    let ptr_ty = cg.context.ptr_type(inkwell::AddressSpace::default());

    if cg.module.get_function("strlen").is_none() {
        let strlen_ty = i64_ty.fn_type(&[ptr_ty.into()], false);
        cg.module.add_function("strlen", strlen_ty, None);
    }
    if cg.module.get_function("strcpy").is_none() {
        let strcpy_ty = ptr_ty.fn_type(&[ptr_ty.into(), ptr_ty.into()], false);
        cg.module.add_function("strcpy", strcpy_ty, None);
    }
    if cg.module.get_function("strcat").is_none() {
        let strcat_ty = ptr_ty.fn_type(&[ptr_ty.into(), ptr_ty.into()], false);
        cg.module.add_function("strcat", strcat_ty, None);
    }
    if cg.module.get_function("malloc").is_none() {
        let malloc_ty = ptr_ty.fn_type(&[i64_ty.into()], false);
        cg.module.add_function("malloc", malloc_ty, None);
    }

    let strlen = cg.module.get_function("strlen").unwrap();
    let ll = cg.builder.build_call(strlen, &[l.into()], "llen").unwrap().try_as_basic_value().unwrap_basic().into_int_value();
    let lr = cg.builder.build_call(strlen, &[r.into()], "rlen").unwrap().try_as_basic_value().unwrap_basic().into_int_value();
    let total = cg.builder.build_int_add(ll, lr, "tot").unwrap();
    let size = cg.builder.build_int_add(total, i64_ty.const_int(1, false), "sz").unwrap();

    let buf = cg.builder.build_call(cg.module.get_function("malloc").unwrap(), &[size.into()], "buf").unwrap()
        .try_as_basic_value().unwrap_basic().into_pointer_value();

    cg.builder.build_call(cg.module.get_function("strcpy").unwrap(), &[buf.into(), l.into()], "cpy").unwrap();
    cg.builder.build_call(cg.module.get_function("strcat").unwrap(), &[buf.into(), r.into()], "cat").unwrap();
    Ok(buf.into())
}

fn compile_assign<'ctx>(cg: &mut CodeGen<'ctx>, left: &Expr, right: &Expr, span: Span) -> Result<BasicValueEnum<'ctx>, AzError> {
    match left {
        Expr::Ident(i) => {
            let var_name = i.name.clone();
            let rhs = cg.compile_expr(right)?;
            match cg.variables.get(&var_name) {
                Some((ptr, _)) => { cg.builder.build_store(*ptr, rhs).unwrap(); Ok(rhs) }
                None => Err(AzError::new(ErrorKind::Semantic, span, format!("undefined '{}'", var_name)))
            }
        }
        Expr::FieldAccess { obj, field, null_safe: _ } => {
            let _ = cg.compile_expr(obj)?;
            let obj_ptr = match obj.as_ref() {
                Expr::Ident(i) => cg.variables.get(&i.name).map(|(ptr, _)| *ptr),
                _ => None,
            };
            match obj_ptr {
                Some(ptr) => {
                    let loaded = cg.builder.build_load(cg.context.ptr_type(inkwell::AddressSpace::default()), ptr, "obj").unwrap().into_pointer_value();
                    let rhv = cg.compile_expr(right)?;
                    let mut found = false;
                    let result = Ok(rhv);
                    for (_, info) in &cg.struct_types {
                        if let Some(idx) = info.field_names.iter().position(|f| f == field) {
                            if info.field_types.get(idx).is_some() {
                                let gep = cg.builder.build_struct_gep(info.llvm_struct, loaded, idx as u32, field).unwrap();
                                cg.builder.build_store(gep, rhv).unwrap();
                                found = true;
                                break;
                            }
                        }
                    }
                    if found { result } else { Err(AzError::new(ErrorKind::Semantic, span, format!("no field '{}'", field))) }
                }
                None => Err(AzError::new(ErrorKind::Semantic, span, "cannot assign to field of non-variable")),
            }
        }
        _ => Err(AzError::new(ErrorKind::Semantic, span, "left side must be a variable or field")),
    }
}
