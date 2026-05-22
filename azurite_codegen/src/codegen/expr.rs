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
        Expr::Bool(b) => Ok(cg.context.bool_type().const_int(*b as u64, false).into()),
        Expr::Null => Ok(cg.context.i64_type().const_zero().into()),
        Expr::Char(c) => Ok(cg.context.i64_type().const_int(*c as u64, false).into()),
        Expr::Self_ => {
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
            } else {
                Err(AzError::new(ErrorKind::Semantic, ident.span, format!("undefined '{}'", ident.name)))
            }
        }
        Expr::Binary { left, op, right } => {
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

            if callee_name == "print" || callee_name == "println" {
                return super::builtin::compile_print(cg, &callee_name, args);
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
            cg.builder.build_unconditional_branch(merge_bb).unwrap();
            cg.builder.position_at_end(else_bb);
            if let Some(e) = else_branch { cg.compile_block_stmts(e, false)?; }
            cg.builder.build_unconditional_branch(merge_bb).unwrap();
            cg.builder.position_at_end(merge_bb);
            Ok(cg.context.i64_type().const_zero().into())
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
            cg.builder.build_unconditional_branch(cond_bb).unwrap();
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
        _ => Err(AzError::new(ErrorKind::Semantic, Span::new(0, 0, 0, 0), "type mismatch")),
    }
}
