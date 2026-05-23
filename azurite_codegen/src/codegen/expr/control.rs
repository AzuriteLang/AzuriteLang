use azurite_errors::{AzError, ErrorKind};
use azurite_parser::ast::*;
use inkwell::values::BasicValueEnum;
use crate::codegen::CodeGen;

pub fn compile_control<'ctx>(cg: &mut CodeGen<'ctx>, expr: &Expr) -> Result<BasicValueEnum<'ctx>, AzError> {
    match expr {
        Expr::If { condition, then_branch, else_branch } => compile_if(cg, condition, then_branch, else_branch.as_deref()),
        Expr::While { condition, body } => compile_while(cg, condition, body),
        Expr::Match { value, arms } => compile_match(cg, value, arms),
        Expr::Block(stmts) => {
            let mut last = None;
            for stmt in stmts { last = cg.compile_stmt(stmt, false)?.or(last); }
            last.ok_or_else(|| AzError::new(ErrorKind::Semantic, expr.span(), "empty block"))
        }
        Expr::Array(elems) => compile_array(cg, elems),
        Expr::Index { obj, index } => compile_index(cg, obj, index),
        Expr::Range { start, end } => { let _ = cg.compile_expr(start)?; cg.compile_expr(end) }
        Expr::EnumVariant { variant, .. } => {
            let tag = variant.as_bytes().iter().fold(0u64, |acc, b| acc.wrapping_add(*b as u64));
            Ok(cg.context.i8_type().const_int(tag % 256, false).into())
        }
        Expr::FieldAccess { obj, field } => {
            let obj_val = cg.compile_expr(obj)?;
            let ptr = obj_val.into_pointer_value();
            for (_, info) in &cg.struct_types {
                if let Some(idx) = info.field_names.iter().position(|f| f == field) {
                    if let Some(ft) = info.field_types.get(idx) {
                        let gep = cg.builder.build_struct_gep(info.llvm_struct, ptr, idx as u32, field).unwrap();
                        return Ok(cg.builder.build_load(*ft, gep, field).unwrap());
                    }
                }
            }
            Ok(cg.context.i64_type().const_zero().into())
        }
        _ => unreachable!(),
    }
}

fn compile_if<'ctx>(cg: &mut CodeGen<'ctx>, condition: &Expr, then_branch: &Expr, else_branch: Option<&Expr>) -> Result<BasicValueEnum<'ctx>, AzError> {
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
    if let Some(el) = else_branch { cg.compile_block_stmts(el, false)?; }
    if !cg.has_terminator() { cg.builder.build_unconditional_branch(merge_bb).unwrap(); }
    cg.builder.position_at_end(merge_bb);
    Ok(cg.context.i64_type().const_zero().into())
}

fn compile_while<'ctx>(cg: &mut CodeGen<'ctx>, condition: &Expr, body: &Expr) -> Result<BasicValueEnum<'ctx>, AzError> {
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

fn compile_match<'ctx>(cg: &mut CodeGen<'ctx>, value: &Expr, arms: &[MatchArm]) -> Result<BasicValueEnum<'ctx>, AzError> {
    let val = cg.compile_expr(value)?.into_int_value();
    let i64_ty = cg.context.i64_type();
    let cf = cg.function.unwrap();
    let after_bb = cg.context.append_basic_block(cf, "match_after");
    let start_block = cg.builder.get_insert_block().unwrap();

    for (i, arm) in arms.iter().enumerate() {
        let is_last = i + 1 == arms.len();
        let arm_bb = cg.context.append_basic_block(cf, &format!("match_arm{}", i));
        if !is_last {
            let rest_bb = cg.context.append_basic_block(cf, &format!("match_rest{}", i));
            let arm_val = i64_ty.const_int(i as u64, false);
            if i == 0 { cg.builder.position_at_end(start_block); }
            let cmp = cg.builder.build_int_compare(inkwell::IntPredicate::EQ, val, arm_val, "mc").unwrap();
            cg.builder.build_conditional_branch(cmp, arm_bb, rest_bb).unwrap();
            cg.builder.position_at_end(arm_bb);
            cg.compile_expr(&arm.body)?;
            if !cg.has_terminator() { cg.builder.build_unconditional_branch(after_bb).unwrap(); }
            cg.builder.position_at_end(rest_bb);
        } else {
            cg.builder.build_unconditional_branch(arm_bb).unwrap();
            cg.builder.position_at_end(arm_bb);
            cg.compile_expr(&arm.body)?;
            if !cg.has_terminator() { cg.builder.build_unconditional_branch(after_bb).unwrap(); }
        }
    }
    cg.builder.position_at_end(after_bb);
    Ok(i64_ty.const_zero().into())
}

fn compile_array<'ctx>(cg: &mut CodeGen<'ctx>, elems: &[Expr]) -> Result<BasicValueEnum<'ctx>, AzError> {
    let count = elems.len() as u32;
    if count == 0 { return Ok(cg.context.i64_type().const_zero().into()); }
    let i64_ty = cg.context.i64_type();
    let size = i64_ty.const_int(count as u64, false);
    let ptr = cg.builder.build_array_malloc(i64_ty, size, "arr").unwrap();
    for (i, elem) in elems.iter().enumerate() {
        let val = cg.compile_expr(elem)?;
        let gep = unsafe { cg.builder.build_gep(i64_ty, ptr, &[cg.context.i32_type().const_int(i as u64, false)], "idx").unwrap() };
        cg.builder.build_store(gep, val).unwrap();
    }
    Ok(ptr.into())
}

fn compile_index<'ctx>(cg: &mut CodeGen<'ctx>, obj: &Expr, index: &Expr) -> Result<BasicValueEnum<'ctx>, AzError> {
    let obj_val = cg.compile_expr(obj)?;
    let idx_val = cg.compile_expr(index)?;
    let ptr = obj_val.into_pointer_value();
    let idx_int = idx_val.into_int_value();
    let elem = unsafe { cg.builder.build_gep(cg.context.i64_type(), ptr, &[idx_int], "elem").unwrap() };
    Ok(cg.builder.build_load(cg.context.i64_type(), elem, "loaded").unwrap())
}
