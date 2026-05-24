use azurite_errors::{AzError, ErrorKind};
use azurite_parser::ast::*;
use inkwell::values::{BasicValueEnum, PointerValue, IntValue};
use inkwell::types::BasicTypeEnum;
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
        Expr::Slice { obj, start, end, end_is_len } => compile_slice(cg, obj, start, end, *end_is_len),
        Expr::Range { start, end } => { let _ = cg.compile_expr(start)?; cg.compile_expr(end) }
        Expr::EnumVariant { variant, .. } => {
            let tag = variant.as_bytes().iter().fold(0u64, |acc, b| acc.wrapping_add(*b as u64));
            Ok(cg.context.i8_type().const_int(tag % 256, false).into())
        }
        Expr::FieldAccess { obj, field, null_safe } => {
            // Check for enum variant access: Color.Red
            if let Expr::Ident(ident) = obj.as_ref() {
                if let Some(variants) = cg.enums.get(&ident.name) {
                    if let Some(idx) = variants.iter().position(|v| v.name.name == *field) {
                        let tag = cg.context.i64_type().const_int(idx as u64, false);
                        if *null_safe { return Ok(tag.into()); }
                        return Ok(tag.into());
                    }
                }
            }
            let obj_val = cg.compile_expr(obj)?;
            let ptr = obj_val.into_pointer_value();
            if *null_safe {
                let is_null = cg.builder.build_is_null(ptr, "is_null").unwrap();
                let cf = cg.function.unwrap();
                let skip_bb = cg.context.append_basic_block(cf, "field_skip");
                let cont_bb = cg.context.append_basic_block(cf, "field_cont");
                let merge_bb = cg.context.append_basic_block(cf, "field_merge");
                cg.builder.build_conditional_branch(is_null, skip_bb, cont_bb).unwrap();
                cg.builder.position_at_end(cont_bb);
                let field_val = {
                    let mut found = None;
                    for (_, info) in &cg.struct_types {
                        if let Some(idx) = info.field_names.iter().position(|f| f == field) {
                            if let Some(ft) = info.field_types.get(idx) {
                                let gep = cg.builder.build_struct_gep(info.llvm_struct, ptr, idx as u32, field).unwrap();
                                found = Some(cg.builder.build_load(*ft, gep, field).unwrap());
                                break;
                            }
                        }
                    }
                    found.unwrap_or_else(|| cg.context.i64_type().const_zero().into())
                };
                cg.builder.build_unconditional_branch(merge_bb).unwrap();
                cg.builder.position_at_end(skip_bb);
                cg.builder.build_unconditional_branch(merge_bb).unwrap();
                cg.builder.position_at_end(merge_bb);
                // Use phi to merge: cont_bb provides field_val, skip_bb provides 0
                // Since i64 is trivial, use alloca-based phi
                let phi = cg.builder.build_phi(cg.context.i64_type(), "ns_phi").unwrap();
                phi.add_incoming(&[(&field_val.into_int_value(), cont_bb), (&cg.context.i64_type().const_zero(), skip_bb)]);
                return Ok(phi.as_basic_value().into());
            }
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
        Expr::Tuple(elems) => {
            let i64_ty = cg.context.i64_type();
            let count = elems.len() as u32;
            if count == 0 { return Ok(i64_ty.const_zero().into()); }
            let struct_ty = cg.context.opaque_struct_type("__tuple");
            let field_types: Vec<inkwell::types::BasicTypeEnum> = vec![i64_ty.into(); count as usize];
            struct_ty.set_body(&field_types, false);
            let alloca = cg.builder.build_alloca(struct_ty, "tuple").unwrap();
            for (i, elem) in elems.iter().enumerate() {
                let val = cg.compile_expr(elem)?;
                let ptr = cg.builder.build_struct_gep(struct_ty, alloca, i as u32, "field").unwrap();
                cg.builder.build_store(ptr, val).unwrap();
            }
            Ok(alloca.into())
        }
        _ => unreachable!(),
    }
}

fn compile_if<'ctx>(cg: &mut CodeGen<'ctx>, condition: &Expr, then_branch: &Expr, else_branch: Option<&Expr>) -> Result<BasicValueEnum<'ctx>, AzError> {
    let cond = cg.compile_expr(condition)?;
    let cond_int = cg.to_bool(cond);
    let cf = cg.function.unwrap();
    let i64_ty = cg.context.i64_type();
    let then_bb = cg.context.append_basic_block(cf, "then");
    let else_bb = cg.context.append_basic_block(cf, "else");
    let merge_bb = cg.context.append_basic_block(cf, "ifcont");

    // Alloca for the result (all branches store to it)
    let res = cg.create_entry_alloca(i64_ty.into(), "if_res");

    cg.builder.build_conditional_branch(cond_int, then_bb, else_bb).unwrap();
    cg.builder.position_at_end(then_bb);
    let then_val = cg.compile_block_stmts(then_branch, true)?;
    if !cg.has_terminator() {
        if let Some(v) = then_val { cg.builder.build_store(res, v).unwrap(); }
        cg.builder.build_unconditional_branch(merge_bb).unwrap();
    }
    cg.builder.position_at_end(else_bb);
    if let Some(el) = else_branch {
        let else_val = cg.compile_block_stmts(el, true)?;
        if !cg.has_terminator() {
            if let Some(v) = else_val { cg.builder.build_store(res, v).unwrap(); }
            cg.builder.build_unconditional_branch(merge_bb).unwrap();
        }
    } else {
        if !cg.has_terminator() {
            cg.builder.build_store(res, i64_ty.const_zero()).unwrap();
            cg.builder.build_unconditional_branch(merge_bb).unwrap();
        }
    }
    cg.builder.position_at_end(merge_bb);
    let result = cg.builder.build_load(i64_ty, res, "if_load").unwrap();
    Ok(result)
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
    // Allocate count+1: one extra slot for the length header at [0]
    let alloc_count = i64_ty.const_int(count as u64 + 1, false);
    let raw_ptr = cg.builder.build_array_malloc(i64_ty, alloc_count, "arr_raw").unwrap();
    // Store length at [0]
    let header = unsafe { cg.builder.build_gep(i64_ty, raw_ptr, &[cg.context.i32_type().const_zero()], "hdr").unwrap() };
    cg.builder.build_store(header, i64_ty.const_int(count as u64, false)).unwrap();
    // Data starts at [1]. Return pointer to data (ptr[1])
    let data_ptr = unsafe { cg.builder.build_gep(i64_ty, raw_ptr, &[cg.context.i32_type().const_int(1, false)], "arr").unwrap() };
    for (i, elem) in elems.iter().enumerate() {
        let val = cg.compile_expr(elem)?;
        let val_i64 = val_to_i64(cg, val);
        let gep = unsafe { cg.builder.build_gep(i64_ty, data_ptr, &[cg.context.i32_type().const_int(i as u64, false)], "idx").unwrap() };
        cg.builder.build_store(gep, val_i64).unwrap();
    }
    Ok(data_ptr.into())
}

/// Read array length from heap header at ptr[-1]
pub fn read_array_len<'ctx>(cg: &mut CodeGen<'ctx>, ptr: PointerValue<'ctx>) -> IntValue<'ctx> {
    let i64_ty = cg.context.i64_type();
    let neg_one = i64_ty.const_int(-1i64 as u64, true);
    let hdr = unsafe { cg.builder.build_gep(i64_ty, ptr, &[neg_one], "alen_hdr").unwrap() };
    cg.builder.build_load(i64_ty, hdr, "alen").unwrap().into_int_value()
}

/// Store array length in heap header at ptr[-1]
pub fn write_array_len<'ctx>(cg: &mut CodeGen<'ctx>, ptr: PointerValue<'ctx>, len: IntValue<'ctx>) {
    let i64_ty = cg.context.i64_type();
    let neg_one = i64_ty.const_int(-1i64 as u64, true);
    let hdr = unsafe { cg.builder.build_gep(i64_ty, ptr, &[neg_one], "alen_hdr").unwrap() };
    cg.builder.build_store(hdr, len).unwrap();
}

pub fn val_to_i64<'ctx>(cg: &mut CodeGen<'ctx>, val: BasicValueEnum<'ctx>) -> BasicValueEnum<'ctx> {
    let i64_ty = cg.context.i64_type();
    match val {
        BasicValueEnum::FloatValue(f) => cg.builder.build_bit_cast(f, i64_ty, "f2i").unwrap(),
        BasicValueEnum::PointerValue(p) => cg.builder.build_ptr_to_int(p, i64_ty, "p2i").unwrap().into(),
        v => v,
    }
}

fn i64_to_val<'ctx>(cg: &mut CodeGen<'ctx>, raw: BasicValueEnum<'ctx>, elem_tag: u64) -> BasicValueEnum<'ctx> {
    let f64_ty = cg.context.f64_type();
    let ptr_ty = cg.context.ptr_type(inkwell::AddressSpace::default());
    match elem_tag {
        1 => cg.builder.build_bit_cast(raw.into_int_value(), f64_ty, "i2f").unwrap().into(),
        2 => cg.builder.build_int_to_ptr(raw.into_int_value(), ptr_ty, "i2p").unwrap().into(),
        _ => raw,
    }
}

fn compile_index<'ctx>(cg: &mut CodeGen<'ctx>, obj: &Expr, index: &Expr) -> Result<BasicValueEnum<'ctx>, AzError> {
    let obj_val = cg.compile_expr(obj)?;
    let idx_val = cg.compile_expr(index)?;
    let ptr = obj_val.into_pointer_value();
    let idx_int = idx_val.into_int_value();
    let elem = unsafe { cg.builder.build_gep(cg.context.i64_type(), ptr, &[idx_int], "elem").unwrap() };
    let raw = cg.builder.build_load(cg.context.i64_type(), elem, "loaded").unwrap();
    let elem_tag = elem_tag_for_expr(cg, obj);
    Ok(i64_to_val(cg, raw, elem_tag))
}

fn elem_tag_for_expr<'ctx>(cg: &CodeGen<'ctx>, obj: &Expr) -> u64 {
    match obj {
        Expr::Ident(ident) => cg.array_elem_types.get(&ident.name).copied().unwrap_or(0),
        Expr::Array(elems) => {
            if let Some(first) = elems.first() {
                match first {
                    Expr::Int(_) => 0,
                    Expr::Float(_) => 1,
                    Expr::String(_) | Expr::Char(_) => 2,
                    Expr::Bool(_) => 3,
                    _ => 0,
                }
            } else { 0 }
        }
        _ => 0,
    }
}

fn compile_slice<'ctx>(cg: &mut CodeGen<'ctx>, obj: &Expr, start: &Expr, end: &Expr, end_is_len: bool) -> Result<BasicValueEnum<'ctx>, AzError> {
    let is_array = matches!(obj, Expr::Ident(ident) if cg.array_elem_types.contains_key(&ident.name));
    let obj_val = cg.compile_expr(obj)?;
    let ptr = obj_val.into_pointer_value();
    let i64_ty = cg.context.i64_type();
    let elem_ty: BasicTypeEnum = if is_array { i64_ty.into() } else { cg.context.i8_type().into() };
    let i64_zero = i64_ty.const_zero();

    let total_len = if is_array {
        if let Expr::Ident(ident) = obj {
            let len_ptr = cg.array_lengths.get(&ident.name).copied().unwrap();
            cg.builder.build_load(i64_ty, len_ptr, "alen").unwrap().into_int_value()
        } else { i64_zero }
    } else {
        if cg.module.get_function("strlen").is_none() {
            let ptr_ty = cg.context.ptr_type(inkwell::AddressSpace::default());
            let ft = i64_ty.fn_type(&[ptr_ty.into()], false);
            cg.module.add_function("strlen", ft, None);
        }
        cg.builder.build_call(
            cg.module.get_function("strlen").unwrap(), &[ptr.into()], "tlen"
        ).unwrap().try_as_basic_value().unwrap_basic().into_int_value()
    };

    let raw_start = cg.compile_expr(start)?.into_int_value();
    let raw_end = if end_is_len { total_len } else { cg.compile_expr(end)?.into_int_value() };

    let sn = cg.builder.build_int_compare(inkwell::IntPredicate::SLT, raw_start, i64_zero, "sn").unwrap();
    let sa = cg.builder.build_int_add(total_len, raw_start, "sa").unwrap();
    let adj_start = cg.builder.build_select(sn, sa, raw_start, "as").unwrap();

    let en = cg.builder.build_int_compare(inkwell::IntPredicate::SLT, raw_end, i64_zero, "en").unwrap();
    let ea = cg.builder.build_int_add(total_len, raw_end, "ea").unwrap();
    let adj_end = cg.builder.build_select(en, ea, raw_end, "ae").unwrap();

    let as_i = adj_start.into_int_value();
    let ae_i = adj_end.into_int_value();
    let lt_s = cg.builder.build_int_compare(inkwell::IntPredicate::SLT, as_i, i64_zero, "lts").unwrap();
    let cs = cg.builder.build_select(lt_s, i64_zero, as_i, "cs").unwrap().into_int_value();
    let gt_s = cg.builder.build_int_compare(inkwell::IntPredicate::SGT, cs, total_len, "gts").unwrap();
    let clamped_start = cg.builder.build_select(gt_s, total_len, cs, "cs2").unwrap().into_int_value();
    let lt_e = cg.builder.build_int_compare(inkwell::IntPredicate::SLT, ae_i, i64_zero, "lte").unwrap();
    let ce = cg.builder.build_select(lt_e, i64_zero, ae_i, "ce").unwrap().into_int_value();
    let gt_e = cg.builder.build_int_compare(inkwell::IntPredicate::SGT, ce, total_len, "gte").unwrap();
    let clamped_end = cg.builder.build_select(gt_e, total_len, ce, "ce2").unwrap().into_int_value();

    let len = cg.builder.build_int_sub(clamped_end, clamped_start, "slen").unwrap();
    let len_pos = cg.builder.build_select(
        cg.builder.build_int_compare(inkwell::IntPredicate::SLT, len, i64_zero, "lz").unwrap(),
        i64_zero, len, "lp"
    ).unwrap().into_int_value();

    let buf = cg.builder.build_array_malloc(elem_ty, len_pos, "slice").unwrap();
    let zero_len = cg.builder.build_int_compare(inkwell::IntPredicate::EQ, len_pos, i64_zero, "zl").unwrap();
    let cf = cg.function.unwrap();
    let copy_bb = cg.context.append_basic_block(cf, "sc_copy");
    let skip_bb = cg.context.append_basic_block(cf, "sc_skip");
    let merge_bb = cg.context.append_basic_block(cf, "sc_end");
    cg.builder.build_conditional_branch(zero_len, skip_bb, copy_bb).unwrap();

    cg.builder.position_at_end(copy_bb);
    let i_ptr = cg.create_entry_alloca(i64_ty.into(), "__si");
    cg.builder.build_store(i_ptr, i64_zero).unwrap();
    let cond_bb = cg.context.append_basic_block(cf, "sl_cond");
    let body_bb = cg.context.append_basic_block(cf, "sl_body");
    let done_bb = cg.context.append_basic_block(cf, "sl_done");
    cg.builder.build_unconditional_branch(cond_bb).unwrap();
    cg.builder.position_at_end(cond_bb);
    let ci = cg.builder.build_load(i64_ty, i_ptr, "ci").unwrap().into_int_value();
    let cc = cg.builder.build_int_compare(inkwell::IntPredicate::SLT, ci, len_pos, "cc").unwrap();
    cg.builder.build_conditional_branch(cc, body_bb, done_bb).unwrap();
    cg.builder.position_at_end(body_bb);
    let ci2 = cg.builder.build_load(i64_ty, i_ptr, "ci2").unwrap().into_int_value();
    let src_idx = cg.builder.build_int_add(ci2, clamped_start, "si").unwrap();
    let src_g = unsafe { cg.builder.build_gep(elem_ty, ptr, &[src_idx], "sg").unwrap() };
    let sv = cg.builder.build_load(elem_ty, src_g, "sv").unwrap();
    let dst_g = unsafe { cg.builder.build_gep(elem_ty, buf, &[ci2], "dg").unwrap() };
    cg.builder.build_store(dst_g, sv).unwrap();
    let ni = cg.builder.build_int_add(ci2, i64_ty.const_int(1, false), "ni").unwrap();
    cg.builder.build_store(i_ptr, ni).unwrap();
    cg.builder.build_unconditional_branch(cond_bb).unwrap();

    cg.builder.position_at_end(done_bb);
    cg.builder.build_unconditional_branch(merge_bb).unwrap();
    cg.builder.position_at_end(skip_bb);
    cg.builder.build_unconditional_branch(merge_bb).unwrap();
    cg.builder.position_at_end(merge_bb);
    // Null-terminate only for strings
    if !is_array {
        let ng = unsafe { cg.builder.build_gep(cg.context.i8_type(), buf, &[len_pos], "ng").unwrap() };
        cg.builder.build_store(ng, cg.context.i8_type().const_zero()).unwrap();
    }

    Ok(buf.into())
}
