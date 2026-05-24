use azurite_errors::{AzError, ErrorKind};
use azurite_lexer::Span;
use azurite_parser::ast::*;
use inkwell::values::{BasicValueEnum, PointerValue, IntValue};
use inkwell::{IntPredicate, FloatPredicate};
use crate::codegen::CodeGen;

// ===== Array helpers =====

pub fn compile_array_contains<'ctx>(cg: &mut CodeGen<'ctx>, ptr: PointerValue<'ctx>, _var_name: &str, val_i64: BasicValueEnum<'ctx>, _elem_tag: u64) -> BasicValueEnum<'ctx> {
    let i64_ty = cg.context.i64_type();
    let neg_one = i64_ty.const_int(-1i64 as u64, true);
    let hdr = unsafe { cg.builder.build_gep(i64_ty, ptr, &[neg_one], "ac_hdr").unwrap() };
    let len = cg.builder.build_load(i64_ty, hdr, "len").unwrap().into_int_value();
    let zero = i64_ty.const_zero();
    let not_empty = cg.builder.build_int_compare(IntPredicate::NE, len, zero, "ne").unwrap();
    let cf = cg.function.unwrap();
    let loop_bb = cg.context.append_basic_block(cf, "c_loop");
    let found_bb = cg.context.append_basic_block(cf, "c_found");
    let not_found_bb = cg.context.append_basic_block(cf, "c_nfound");
    let done_bb = cg.context.append_basic_block(cf, "c_done");
    let result_alloca = cg.create_entry_alloca(i64_ty.into(), "c_res");
    cg.builder.build_store(result_alloca, zero).unwrap();
    cg.builder.build_conditional_branch(not_empty, loop_bb, done_bb).unwrap();
    cg.builder.position_at_end(loop_bb);
    let i_ptr = cg.create_entry_alloca(i64_ty.into(), "__ci");
    cg.builder.build_store(i_ptr, zero).unwrap();
    let cond_bb = cg.context.append_basic_block(cf, "c_cond");
    let body_bb = cg.context.append_basic_block(cf, "c_body");
    let next_bb = cg.context.append_basic_block(cf, "c_next");
    cg.builder.build_unconditional_branch(cond_bb).unwrap();
    cg.builder.position_at_end(cond_bb);
    let ci = cg.builder.build_load(i64_ty, i_ptr, "ci").unwrap().into_int_value();
    let cc = cg.builder.build_int_compare(IntPredicate::SLT, ci, len, "cc").unwrap();
    cg.builder.build_conditional_branch(cc, body_bb, not_found_bb).unwrap();
    cg.builder.position_at_end(body_bb);
    let gep = unsafe { cg.builder.build_gep(i64_ty, ptr, &[ci], "c_gep").unwrap() };
    let elem = cg.builder.build_load(i64_ty, gep, "c_elem").unwrap();
    let eq = cg.builder.build_int_compare(IntPredicate::EQ, elem.into_int_value(), val_i64.into_int_value(), "ceq").unwrap();
    cg.builder.build_conditional_branch(eq, found_bb, next_bb).unwrap();
    cg.builder.position_at_end(next_bb);
    let ni = cg.builder.build_int_add(ci, i64_ty.const_int(1, false), "ni").unwrap();
    cg.builder.build_store(i_ptr, ni).unwrap();
    cg.builder.build_unconditional_branch(cond_bb).unwrap();
    cg.builder.position_at_end(found_bb);
    cg.builder.build_store(result_alloca, i64_ty.const_int(1, false)).unwrap();
    cg.builder.build_unconditional_branch(done_bb).unwrap();
    cg.builder.position_at_end(not_found_bb);
    cg.builder.build_unconditional_branch(done_bb).unwrap();
    cg.builder.position_at_end(done_bb);
    cg.builder.build_load(i64_ty, result_alloca, "c_r").unwrap()
}

pub fn compile_array_reverse<'ctx>(cg: &mut CodeGen<'ctx>, _var_name: &str, ptr: PointerValue<'ctx>, _elem_tag: u64) {
    let i64_ty = cg.context.i64_type();
    let neg_one = i64_ty.const_int(-1i64 as u64, true);
    let hdr = unsafe { cg.builder.build_gep(i64_ty, ptr, &[neg_one], "ar_hdr").unwrap() };
    let len = cg.builder.build_load(i64_ty, hdr, "len").unwrap().into_int_value();
    let zero = i64_ty.const_zero();
    let one = i64_ty.const_int(1, false);
    let gt1 = cg.builder.build_int_compare(IntPredicate::SGT, len, one, "gt1").unwrap();
    let cf = cg.function.unwrap();
    let loop_bb = cg.context.append_basic_block(cf, "rv_loop");
    let done_bb = cg.context.append_basic_block(cf, "rv_done");
    cg.builder.build_conditional_branch(gt1, loop_bb, done_bb).unwrap();
    cg.builder.position_at_end(loop_bb);
    let i_ptr = cg.create_entry_alloca(i64_ty.into(), "__rvi");
    cg.builder.build_store(i_ptr, zero).unwrap();
    let half = cg.builder.build_int_signed_div(len, i64_ty.const_int(2, false), "half").unwrap();
    let cond_bb = cg.context.append_basic_block(cf, "rv_cond");
    let body_bb = cg.context.append_basic_block(cf, "rv_body");
    let _inc_bb = cg.context.append_basic_block(cf, "rv_inc");
    cg.builder.build_unconditional_branch(cond_bb).unwrap();
    cg.builder.position_at_end(cond_bb);
    let ci = cg.builder.build_load(i64_ty, i_ptr, "ci").unwrap().into_int_value();
    let cc = cg.builder.build_int_compare(IntPredicate::SLT, ci, half, "cc").unwrap();
    cg.builder.build_conditional_branch(cc, body_bb, done_bb).unwrap();
    cg.builder.position_at_end(body_bb);
    let j = cg.builder.build_int_sub(cg.builder.build_int_sub(len, one, "lm1").unwrap(), ci, "j").unwrap();
    let g1 = unsafe { cg.builder.build_gep(i64_ty, ptr, &[ci], "rg1").unwrap() };
    let g2 = unsafe { cg.builder.build_gep(i64_ty, ptr, &[j], "rg2").unwrap() };
    let v1 = cg.builder.build_load(i64_ty, g1, "rv1").unwrap();
    let v2 = cg.builder.build_load(i64_ty, g2, "rv2").unwrap();
    cg.builder.build_store(g1, v2).unwrap();
    cg.builder.build_store(g2, v1).unwrap();
    let ni = cg.builder.build_int_add(ci, one, "ni").unwrap();
    cg.builder.build_store(i_ptr, ni).unwrap();
    cg.builder.build_unconditional_branch(cond_bb).unwrap();
    cg.builder.position_at_end(done_bb);
}

pub fn compile_array_sort<'ctx>(cg: &mut CodeGen<'ctx>, _var_name: &str, ptr: PointerValue<'ctx>, elem_tag: u64) {
    let i64_ty = cg.context.i64_type();
    let neg_one = i64_ty.const_int(-1i64 as u64, true);
    let hdr = unsafe { cg.builder.build_gep(i64_ty, ptr, &[neg_one], "as_hdr").unwrap() };
    let len = cg.builder.build_load(i64_ty, hdr, "len").unwrap().into_int_value();
    let one = i64_ty.const_int(1, false);
    let zero = i64_ty.const_zero();
    let gt1 = cg.builder.build_int_compare(IntPredicate::SGT, len, one, "gt1").unwrap();
    let cf = cg.function.unwrap();
    let sort_bb = cg.context.append_basic_block(cf, "st_body");
    let done_bb = cg.context.append_basic_block(cf, "st_done");
    cg.builder.build_conditional_branch(gt1, sort_bb, done_bb).unwrap();
    cg.builder.position_at_end(sort_bb);
    let i_ptr = cg.create_entry_alloca(i64_ty.into(), "__sti");
    cg.builder.build_store(i_ptr, zero).unwrap();
    let outer_cond = cg.context.append_basic_block(cf, "st_ocond");
    let outer_body = cg.context.append_basic_block(cf, "st_obody");
    let outer_inc = cg.context.append_basic_block(cf, "st_oinc");
    cg.builder.build_unconditional_branch(outer_cond).unwrap();
    cg.builder.position_at_end(outer_cond);
    let oci = cg.builder.build_load(i64_ty, i_ptr, "oci").unwrap().into_int_value();
    let last = cg.builder.build_int_sub(len, one, "lst").unwrap();
    let occ = cg.builder.build_int_compare(IntPredicate::SLT, oci, last, "occ").unwrap();
    cg.builder.build_conditional_branch(occ, outer_body, done_bb).unwrap();
    cg.builder.position_at_end(outer_body);
    let j_ptr = cg.create_entry_alloca(i64_ty.into(), "__stj");
    let j_init = cg.builder.build_int_add(oci, one, "jp1").unwrap();
    cg.builder.build_store(j_ptr, j_init).unwrap();
    let inner_cond = cg.context.append_basic_block(cf, "st_icond");
    let inner_body = cg.context.append_basic_block(cf, "st_ibody");
    let inner_inc = cg.context.append_basic_block(cf, "st_iinc");
    cg.builder.build_unconditional_branch(inner_cond).unwrap();
    cg.builder.position_at_end(inner_cond);
    let ici = cg.builder.build_load(i64_ty, j_ptr, "ici").unwrap().into_int_value();
    let icc = cg.builder.build_int_compare(IntPredicate::SLT, ici, len, "icc").unwrap();
    cg.builder.build_conditional_branch(icc, inner_body, outer_inc).unwrap();
    cg.builder.position_at_end(inner_body);
    let gi = unsafe { cg.builder.build_gep(i64_ty, ptr, &[oci], "sgi").unwrap() };
    let gj = unsafe { cg.builder.build_gep(i64_ty, ptr, &[ici], "sgj").unwrap() };
    let vi = cg.builder.build_load(i64_ty, gi, "svi").unwrap().into_int_value();
    let vj = cg.builder.build_load(i64_ty, gj, "svj").unwrap().into_int_value();
    let cmp = if elem_tag == 1 {
        let fi = cg.builder.build_bit_cast(vi, cg.context.f64_type(), "i2f_i").unwrap().into_float_value();
        let fj = cg.builder.build_bit_cast(vj, cg.context.f64_type(), "i2f_j").unwrap().into_float_value();
        cg.builder.build_float_compare(FloatPredicate::OGT, fi, fj, "fcmp").unwrap()
    } else {
        cg.builder.build_int_compare(IntPredicate::SGT, vi, vj, "icmp").unwrap()
    };
    let swap_bb = cg.context.append_basic_block(cf, "st_swap");
    let no_swap_bb = cg.context.append_basic_block(cf, "st_nosw");
    cg.builder.build_conditional_branch(cmp, swap_bb, no_swap_bb).unwrap();
    cg.builder.position_at_end(swap_bb);
    cg.builder.build_store(gi, vj).unwrap();
    cg.builder.build_store(gj, vi).unwrap();
    cg.builder.build_unconditional_branch(no_swap_bb).unwrap();
    cg.builder.position_at_end(no_swap_bb);
    cg.builder.build_unconditional_branch(inner_inc).unwrap();
    cg.builder.position_at_end(inner_inc);
    let nji = cg.builder.build_int_add(ici, one, "nji").unwrap();
    cg.builder.build_store(j_ptr, nji).unwrap();
    cg.builder.build_unconditional_branch(inner_cond).unwrap();
    cg.builder.position_at_end(outer_inc);
    let noi = cg.builder.build_int_add(oci, one, "noi").unwrap();
    cg.builder.build_store(i_ptr, noi).unwrap();
    cg.builder.build_unconditional_branch(outer_cond).unwrap();
    cg.builder.position_at_end(done_bb);
}

fn array_len_ptr<'ctx>(cg: &CodeGen<'ctx>, ptr: PointerValue<'ctx>) -> PointerValue<'ctx> {
    let neg_one = cg.context.i64_type().const_int(-1i64 as u64, true);
    unsafe { cg.builder.build_gep(cg.context.i64_type(), ptr, &[neg_one], "alen").unwrap() }
}

fn read_array_len<'ctx>(cg: &mut CodeGen<'ctx>, ptr: PointerValue<'ctx>) -> IntValue<'ctx> {
    let hdr = array_len_ptr(cg, ptr);
    cg.builder.build_load(cg.context.i64_type(), hdr, "rlen").unwrap().into_int_value()
}

fn write_array_len<'ctx>(cg: &mut CodeGen<'ctx>, ptr: PointerValue<'ctx>, len: IntValue<'ctx>) {
    let hdr = array_len_ptr(cg, ptr);
    cg.builder.build_store(hdr, len).unwrap();
}

pub fn compile_array_insert<'ctx>(cg: &mut CodeGen<'ctx>, var_name: &str, ptr: PointerValue<'ctx>, _elem_tag: u64, idx_expr: &Expr, val_expr: &Expr) -> Result<(), AzError> {
    let i64_ty = cg.context.i64_type();
    let val = cg.compile_expr(val_expr)?;
    let val_i64 = super::control::val_to_i64(cg, val);
    let idx = cg.compile_expr(idx_expr)?.into_int_value();
    let old_len = read_array_len(cg, ptr);
    let cap_name = format!("{}.__cap", var_name);
    // If we have a capacity entry, use it for realloc tracking
    if let Some(&cap_ptr) = cg.array_lengths.get(&cap_name) {
        let old_cap = cg.builder.build_load(i64_ty, cap_ptr, "oc").unwrap().into_int_value();
        let full = cg.builder.build_int_compare(IntPredicate::EQ, old_len, old_cap, "full").unwrap();
        let cf = cg.function.unwrap();
        let grow_bb = cg.context.append_basic_block(cf, "igrow");
        let skip_bb = cg.context.append_basic_block(cf, "inogrow");
        let merge_bb = cg.context.append_basic_block(cf, "imerge");
        cg.builder.build_conditional_branch(full, grow_bb, skip_bb).unwrap();
        cg.builder.position_at_end(grow_bb);
        let zero_cap = cg.builder.build_int_compare(IntPredicate::EQ, old_cap, i64_ty.const_zero(), "zc").unwrap();
        let base_cap = i64_ty.const_int(4, false);
        let doubled = cg.builder.build_int_mul(old_cap, i64_ty.const_int(2, false), "dbl").unwrap();
        let new_cap = cg.builder.build_select(zero_cap, base_cap, doubled, "nc").unwrap().into_int_value();
        let new_sz = cg.builder.build_int_mul(new_cap, i64_ty.const_int(8, false), "nsz").unwrap();
        if cg.module.get_function("realloc").is_none() {
            let ptr_ty = cg.context.ptr_type(inkwell::AddressSpace::default());
            let rt = ptr_ty.fn_type(&[ptr_ty.into(), i64_ty.into()], false);
            cg.module.add_function("realloc", rt, None);
        }
        // realloc must account for the header slot too
        let hdr_sz = i64_ty.const_int(8, false);
        let realloc_sz = cg.builder.build_int_add(new_sz, hdr_sz, "rsz").unwrap();
        let raw_ptr = cg.builder.build_int_to_ptr(cg.builder.build_ptr_to_int(ptr, i64_ty, "p2i").unwrap(), cg.context.ptr_type(inkwell::AddressSpace::default()), "p2p").unwrap();
        let realloc_args = &[raw_ptr.into(), realloc_sz.into()];
        let new_raw = cg.builder.build_call(cg.module.get_function("realloc").unwrap(), realloc_args, "rp").unwrap();
        let new_raw_pv = new_raw.try_as_basic_value().unwrap_basic().into_pointer_value();
        // data starts at new_raw + 1
        let new_data = unsafe { cg.builder.build_gep(i64_ty, new_raw_pv, &[i64_ty.const_int(1, false)], "nd").unwrap() };
        cg.builder.build_store(cap_ptr, new_cap).unwrap();
        if let Some((arr_alloca, _)) = cg.variables.get(var_name) {
            cg.builder.build_store(*arr_alloca, new_data).unwrap();
        }
        cg.builder.build_unconditional_branch(merge_bb).unwrap();
        cg.builder.position_at_end(skip_bb);
        cg.builder.build_unconditional_branch(merge_bb).unwrap();
        cg.builder.position_at_end(merge_bb);
    }
    let arr_final = if let Some((arr_alloca, _)) = cg.variables.get(var_name) {
        cg.builder.build_load(cg.context.ptr_type(inkwell::AddressSpace::default()), *arr_alloca, "arr_f").unwrap().into_pointer_value()
    } else { ptr };
    // Shift elements right from idx
    let i_ptr = cg.create_entry_alloca(i64_ty.into(), "__ini");
    cg.builder.build_store(i_ptr, old_len).unwrap();
    let cf = cg.function.unwrap();
    let cond_bb = cg.context.append_basic_block(cf, "icond");
    let body_bb = cg.context.append_basic_block(cf, "ibody");
    let done_bb = cg.context.append_basic_block(cf, "idone");
    cg.builder.build_unconditional_branch(cond_bb).unwrap();
    cg.builder.position_at_end(cond_bb);
    let ci = cg.builder.build_load(i64_ty, i_ptr, "ci").unwrap().into_int_value();
    let cc = cg.builder.build_int_compare(IntPredicate::SGT, ci, idx, "icc").unwrap();
    cg.builder.build_conditional_branch(cc, body_bb, done_bb).unwrap();
    cg.builder.position_at_end(body_bb);
    let prev = cg.builder.build_int_sub(ci, i64_ty.const_int(1, false), "pr").unwrap();
    let src_gep = unsafe { cg.builder.build_gep(i64_ty, arr_final, &[prev], "isrc").unwrap() };
    let dst_gep = unsafe { cg.builder.build_gep(i64_ty, arr_final, &[ci], "idst").unwrap() };
    let tmp = cg.builder.build_load(i64_ty, src_gep, "itmp").unwrap();
    cg.builder.build_store(dst_gep, tmp).unwrap();
    let ni = cg.builder.build_int_sub(ci, i64_ty.const_int(1, false), "ni").unwrap();
    cg.builder.build_store(i_ptr, ni).unwrap();
    cg.builder.build_unconditional_branch(cond_bb).unwrap();
    cg.builder.position_at_end(done_bb);
    let ins_gep = unsafe { cg.builder.build_gep(i64_ty, arr_final, &[idx], "iins").unwrap() };
    cg.builder.build_store(ins_gep, val_i64).unwrap();
    let new_len = cg.builder.build_int_add(old_len, i64_ty.const_int(1, false), "nl").unwrap();
    write_array_len(cg, ptr, new_len);
    // Also update array_lengths entry if present
    if let Some(&len_ptr) = cg.array_lengths.get(var_name) {
        cg.builder.build_store(len_ptr, new_len).unwrap();
    }
    Ok(())
}

pub fn compile_array_remove<'ctx>(cg: &mut CodeGen<'ctx>, var_name: &str, ptr: PointerValue<'ctx>, _elem_tag: u64, idx_expr: &Expr) -> Result<BasicValueEnum<'ctx>, AzError> {
    let i64_ty = cg.context.i64_type();
    let idx = cg.compile_expr(idx_expr)?.into_int_value();
    let old_len = read_array_len(cg, ptr);
    let one = i64_ty.const_int(1, false);
    let gep = unsafe { cg.builder.build_gep(i64_ty, ptr, &[idx], "rgep").unwrap() };
    let result = cg.builder.build_load(i64_ty, gep, "rval").unwrap();
    let cf = cg.function.unwrap();
    let i_ptr = cg.create_entry_alloca(i64_ty.into(), "__rni");
    let ci_start = cg.builder.build_int_add(idx, one, "is").unwrap();
    cg.builder.build_store(i_ptr, ci_start).unwrap();
    let cond_bb = cg.context.append_basic_block(cf, "rcond");
    let body_bb = cg.context.append_basic_block(cf, "rbody");
    let done_bb = cg.context.append_basic_block(cf, "rdone");
    cg.builder.build_unconditional_branch(cond_bb).unwrap();
    cg.builder.position_at_end(cond_bb);
    let ci = cg.builder.build_load(i64_ty, i_ptr, "ci").unwrap().into_int_value();
    let cc = cg.builder.build_int_compare(IntPredicate::SLT, ci, old_len, "rcc").unwrap();
    cg.builder.build_conditional_branch(cc, body_bb, done_bb).unwrap();
    cg.builder.position_at_end(body_bb);
    let src_gep = unsafe { cg.builder.build_gep(i64_ty, ptr, &[ci], "rsrc").unwrap() };
    let dst_gep = unsafe { cg.builder.build_gep(i64_ty, ptr, &[cg.builder.build_int_sub(ci, one, "rd").unwrap()], "rdst").unwrap() };
    let tmp = cg.builder.build_load(i64_ty, src_gep, "rtmp").unwrap();
    cg.builder.build_store(dst_gep, tmp).unwrap();
    let ni = cg.builder.build_int_add(ci, one, "rni").unwrap();
    cg.builder.build_store(i_ptr, ni).unwrap();
    cg.builder.build_unconditional_branch(cond_bb).unwrap();
    cg.builder.position_at_end(done_bb);
    let new_len = cg.builder.build_int_sub(old_len, one, "rnl").unwrap();
    write_array_len(cg, ptr, new_len);
    if let Some(&len_ptr) = cg.array_lengths.get(var_name) {
        cg.builder.build_store(len_ptr, new_len).unwrap();
    }
    Ok(result)
}

// ===== Iterator methods (map, filter, reduce) =====

fn alloc_array_with_len<'ctx>(cg: &mut CodeGen<'ctx>, count: IntValue<'ctx>) -> PointerValue<'ctx> {
    let i64_ty = cg.context.i64_type();
    let one = i64_ty.const_int(1, false);
    let alloc_count = cg.builder.build_int_add(count, one, "acnt").unwrap();
    let raw = cg.builder.build_array_malloc(i64_ty, alloc_count, "arr_raw").unwrap();
    let hdr = unsafe { cg.builder.build_gep(i64_ty, raw, &[i64_ty.const_zero()], "hdr").unwrap() };
    cg.builder.build_store(hdr, count).unwrap();
    unsafe { cg.builder.build_gep(i64_ty, raw, &[one], "data").unwrap() }
}

pub fn compile_array_map<'ctx>(cg: &mut CodeGen<'ctx>, _var_name: &str, ptr: PointerValue<'ctx>, _elem_tag: u64, fn_name: &str) -> Result<BasicValueEnum<'ctx>, AzError> {
    let i64_ty = cg.context.i64_type();
    let count = read_array_len(cg, ptr);
    let fn_val = cg.module.get_function(fn_name).ok_or_else(|| AzError::new(ErrorKind::Semantic, Span::new(0, 0, 0, 0), format!("unknown function '{}'", fn_name)))?;
    let new_ptr = alloc_array_with_len(cg, count);
    let zero = i64_ty.const_zero();
    let one = i64_ty.const_int(1, false);
    let cf = cg.function.unwrap();
    let i_ptr = cg.create_entry_alloca(i64_ty.into(), "__mpi");
    cg.builder.build_store(i_ptr, zero).unwrap();
    let cond_bb = cg.context.append_basic_block(cf, "mp_cond");
    let body_bb = cg.context.append_basic_block(cf, "mp_body");
    let done_bb = cg.context.append_basic_block(cf, "mp_done");
    cg.builder.build_unconditional_branch(cond_bb).unwrap();
    cg.builder.position_at_end(cond_bb);
    let ci = cg.builder.build_load(i64_ty, i_ptr, "ci").unwrap().into_int_value();
    let cc = cg.builder.build_int_compare(IntPredicate::SLT, ci, count, "cc").unwrap();
    cg.builder.build_conditional_branch(cc, body_bb, done_bb).unwrap();
    cg.builder.position_at_end(body_bb);
    let src_gep = unsafe { cg.builder.build_gep(i64_ty, ptr, &[ci], "msrc").unwrap() };
    let elem = cg.builder.build_load(i64_ty, src_gep, "me").unwrap();
    let call_result = cg.builder.build_call(fn_val, &[elem.into()], "mcall").unwrap();
    let result = call_result.try_as_basic_value().unwrap_basic();
    let dst_gep = unsafe { cg.builder.build_gep(i64_ty, new_ptr, &[ci], "mdst").unwrap() };
    cg.builder.build_store(dst_gep, result).unwrap();
    let ni = cg.builder.build_int_add(ci, one, "ni").unwrap();
    cg.builder.build_store(i_ptr, ni).unwrap();
    cg.builder.build_unconditional_branch(cond_bb).unwrap();
    cg.builder.position_at_end(done_bb);
    Ok(new_ptr.into())
}

pub fn compile_array_filter<'ctx>(cg: &mut CodeGen<'ctx>, _var_name: &str, ptr: PointerValue<'ctx>, _elem_tag: u64, fn_name: &str) -> Result<BasicValueEnum<'ctx>, AzError> {
    let i64_ty = cg.context.i64_type();
    let len = read_array_len(cg, ptr);
    let fn_val = cg.module.get_function(fn_name).ok_or_else(|| AzError::new(ErrorKind::Semantic, Span::new(0, 0, 0, 0), format!("unknown function '{}'", fn_name)))?;
    // Allocate output with header (pre-allocate max size)
    let new_ptr = alloc_array_with_len(cg, len);
    let zero = i64_ty.const_zero();
    let one = i64_ty.const_int(1, false);
    let cf = cg.function.unwrap();
    let i_ptr = cg.create_entry_alloca(i64_ty.into(), "__fli");
    let o_ptr = cg.create_entry_alloca(i64_ty.into(), "__flo");
    cg.builder.build_store(i_ptr, zero).unwrap();
    cg.builder.build_store(o_ptr, zero).unwrap();
    let cond_bb = cg.context.append_basic_block(cf, "fl_cond");
    let body_bb = cg.context.append_basic_block(cf, "fl_body");
    let inc_bb = cg.context.append_basic_block(cf, "fl_inc");
    let done_bb = cg.context.append_basic_block(cf, "fl_done");
    cg.builder.build_unconditional_branch(cond_bb).unwrap();
    cg.builder.position_at_end(cond_bb);
    let ci = cg.builder.build_load(i64_ty, i_ptr, "ci").unwrap().into_int_value();
    let cc = cg.builder.build_int_compare(IntPredicate::SLT, ci, len, "cc").unwrap();
    cg.builder.build_conditional_branch(cc, body_bb, done_bb).unwrap();
    cg.builder.position_at_end(body_bb);
    let src_gep = unsafe { cg.builder.build_gep(i64_ty, ptr, &[ci], "fsrc").unwrap() };
    let elem = cg.builder.build_load(i64_ty, src_gep, "fe").unwrap();
    let call_result = cg.builder.build_call(fn_val, &[elem.into()], "fcall").unwrap();
    let keep = call_result.try_as_basic_value().unwrap_basic().into_int_value();
    let is_true = cg.builder.build_int_compare(IntPredicate::NE, keep, zero, "ist").unwrap();
    let store_bb = cg.context.append_basic_block(cf, "fl_store");
    let skip_bb = cg.context.append_basic_block(cf, "fl_skip");
    cg.builder.build_conditional_branch(is_true, store_bb, skip_bb).unwrap();
    cg.builder.position_at_end(store_bb);
    let oi = cg.builder.build_load(i64_ty, o_ptr, "oi").unwrap().into_int_value();
    let dst_gep = unsafe { cg.builder.build_gep(i64_ty, new_ptr, &[oi], "fdst").unwrap() };
    cg.builder.build_store(dst_gep, elem).unwrap();
    let no = cg.builder.build_int_add(oi, one, "no").unwrap();
    cg.builder.build_store(o_ptr, no).unwrap();
    cg.builder.build_unconditional_branch(skip_bb).unwrap();
    cg.builder.position_at_end(skip_bb);
    cg.builder.build_unconditional_branch(inc_bb).unwrap();
    cg.builder.position_at_end(inc_bb);
    let ni = cg.builder.build_int_add(ci, one, "ni").unwrap();
    cg.builder.build_store(i_ptr, ni).unwrap();
    cg.builder.build_unconditional_branch(cond_bb).unwrap();
    cg.builder.position_at_end(done_bb);
    // Update header with actual count
    let o_final = cg.builder.build_load(i64_ty, o_ptr, "of").unwrap().into_int_value();
    write_array_len(cg, new_ptr, o_final);
    Ok(new_ptr.into())
}

pub fn compile_array_reduce<'ctx>(cg: &mut CodeGen<'ctx>, _var_name: &str, ptr: PointerValue<'ctx>, _elem_tag: u64, init_expr: &Expr, fn_name: &str) -> Result<BasicValueEnum<'ctx>, AzError> {
    let i64_ty = cg.context.i64_type();
    let len = read_array_len(cg, ptr);
    let fn_val = cg.module.get_function(fn_name).ok_or_else(|| AzError::new(ErrorKind::Semantic, Span::new(0, 0, 0, 0), format!("unknown function '{}'", fn_name)))?;
    let init = cg.compile_expr(init_expr)?;
    let acc_ptr = cg.create_entry_alloca(i64_ty.into(), "__rdacc");
    cg.builder.build_store(acc_ptr, init).unwrap();
    let zero = i64_ty.const_zero();
    let one = i64_ty.const_int(1, false);
    let cf = cg.function.unwrap();
    let i_ptr = cg.create_entry_alloca(i64_ty.into(), "__rdi");
    cg.builder.build_store(i_ptr, zero).unwrap();
    let cond_bb = cg.context.append_basic_block(cf, "rd_cond");
    let body_bb = cg.context.append_basic_block(cf, "rd_body");
    let done_bb = cg.context.append_basic_block(cf, "rd_done");
    cg.builder.build_unconditional_branch(cond_bb).unwrap();
    cg.builder.position_at_end(cond_bb);
    let ci = cg.builder.build_load(i64_ty, i_ptr, "ci").unwrap().into_int_value();
    let cc = cg.builder.build_int_compare(IntPredicate::SLT, ci, len, "cc").unwrap();
    cg.builder.build_conditional_branch(cc, body_bb, done_bb).unwrap();
    cg.builder.position_at_end(body_bb);
    let src_gep = unsafe { cg.builder.build_gep(i64_ty, ptr, &[ci], "rsrc").unwrap() };
    let elem = cg.builder.build_load(i64_ty, src_gep, "re").unwrap();
    let acc = cg.builder.build_load(i64_ty, acc_ptr, "racc").unwrap();
    let call_result = cg.builder.build_call(fn_val, &[acc.into(), elem.into()], "rcall").unwrap();
    let result = call_result.try_as_basic_value().unwrap_basic();
    cg.builder.build_store(acc_ptr, result).unwrap();
    let ni = cg.builder.build_int_add(ci, one, "ni").unwrap();
    cg.builder.build_store(i_ptr, ni).unwrap();
    cg.builder.build_unconditional_branch(cond_bb).unwrap();
    cg.builder.position_at_end(done_bb);
    let final_acc = cg.builder.build_load(i64_ty, acc_ptr, "rfin").unwrap();
    Ok(final_acc)
}
