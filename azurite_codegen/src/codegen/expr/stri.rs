use azurite_errors::{AzError, ErrorKind};
use azurite_lexer::Span;
use azurite_parser::ast::*;
use inkwell::values::BasicValueEnum;
use crate::codegen::CodeGen;

pub fn dispatch<'ctx>(cg: &mut CodeGen<'ctx>, name: &str, args: &[Expr]) -> Result<BasicValueEnum<'ctx>, AzError> {
    match name {
        "len" => compile_len(cg, args),
        "str" => compile_str(cg, args),
        "int" => compile_int_cast(cg, args),
        "float" => compile_float_cast(cg, args),
        "char_at" => compile_char_at(cg, args),
        "chr" => compile_chr(cg, args),
        _ => Err(AzError::new(ErrorKind::Semantic, Span::new(0, 0, 0, 0), format!("unknown string builtin '{}'", name))),
    }
}

fn compile_len<'ctx>(cg: &mut CodeGen<'ctx>, args: &[Expr]) -> Result<BasicValueEnum<'ctx>, AzError> {
    if let Expr::Array(elems) = &args[0] {
        return Ok(cg.context.i64_type().const_int(elems.len() as u64, false).into());
    }
    if let Expr::Ident(ident) = &args[0] {
        if let Some(len_ptr) = cg.array_lengths.get(&ident.name) {
            return Ok(cg.builder.build_load(cg.context.i64_type(), *len_ptr, "arr_len").unwrap());
        }
    }
    let val = cg.compile_expr(&args[0])?;
    let ptr = val.into_pointer_value();
    if cg.module.get_function("strlen").is_none() {
        let ptr_ty = cg.context.ptr_type(inkwell::AddressSpace::default());
        let strlen_ty = cg.context.i64_type().fn_type(&[ptr_ty.into()], false);
        cg.module.add_function("strlen", strlen_ty, None);
    }
    let len = cg.builder.build_call(cg.module.get_function("strlen").unwrap(), &[ptr.into()], "len").unwrap();
    Ok(match len.try_as_basic_value() { inkwell::values::ValueKind::Basic(bv) => bv, _ => cg.context.i64_type().const_zero().into() })
}

fn compile_int_cast<'ctx>(cg: &mut CodeGen<'ctx>, args: &[Expr]) -> Result<BasicValueEnum<'ctx>, AzError> {
    let val = cg.compile_expr(&args[0])?;
    Ok(cg.builder.build_float_to_signed_int(val.into_float_value(), cg.context.i64_type(), "f2i").unwrap().into())
}

fn compile_float_cast<'ctx>(cg: &mut CodeGen<'ctx>, args: &[Expr]) -> Result<BasicValueEnum<'ctx>, AzError> {
    let val = cg.compile_expr(&args[0])?;
    Ok(cg.builder.build_signed_int_to_float(val.into_int_value(), cg.context.f64_type(), "i2f").unwrap().into())
}

fn compile_char_at<'ctx>(cg: &mut CodeGen<'ctx>, args: &[Expr]) -> Result<BasicValueEnum<'ctx>, AzError> {
    let s = cg.compile_expr(&args[0])?;
    let idx = cg.compile_expr(&args[1])?.into_int_value();
    let ptr = s.into_pointer_value();
    let elem = unsafe { cg.builder.build_gep(cg.context.i8_type(), ptr, &[idx], "ch").unwrap() };
    let loaded = cg.builder.build_load(cg.context.i8_type(), elem, "char").unwrap();
    Ok(cg.builder.build_int_z_extend(loaded.into_int_value(), cg.context.i64_type(), "ch_ext").unwrap().into())
}

fn compile_chr<'ctx>(cg: &mut CodeGen<'ctx>, args: &[Expr]) -> Result<BasicValueEnum<'ctx>, AzError> {
    let val = cg.compile_expr(&args[0])?;
    let i64_val = val.into_int_value();
    let i8_val = cg.builder.build_int_truncate(i64_val, cg.context.i8_type(), "chr_trunc").unwrap();
    if cg.module.get_function("malloc").is_none() {
        let malloc_ty = cg.context.ptr_type(inkwell::AddressSpace::default())
            .fn_type(&[cg.context.i64_type().into()], false);
        cg.module.add_function("malloc", malloc_ty, None);
    }
    let buf = cg.builder.build_call(
        cg.module.get_function("malloc").unwrap(),
        &[cg.context.i64_type().const_int(2, false).into()], "chr_malloc"
    ).unwrap().try_as_basic_value().unwrap_basic().into_pointer_value();
    cg.builder.build_store(buf, i8_val).unwrap();
    let null_gep = unsafe { cg.builder.build_gep(cg.context.i8_type(), buf, &[cg.context.i64_type().const_int(1, false)], "null_gep").unwrap() };
    cg.builder.build_store(null_gep, cg.context.i8_type().const_zero()).unwrap();
    Ok(buf.into())
}

fn compile_str<'ctx>(cg: &mut CodeGen<'ctx>, args: &[Expr]) -> Result<BasicValueEnum<'ctx>, AzError> {
    if args.is_empty() { return Ok(cg.builder.build_global_string_ptr("", "empty_str").unwrap().as_pointer_value().into()); }
    let val = cg.compile_expr(&args[0])?;
    let i64_ty = cg.context.i64_type();
    let i8_ty = cg.context.i8_type();

    match val {
        BasicValueEnum::PointerValue(p) => return Ok(p.into()),
        BasicValueEnum::FloatValue(f) => {
            let ptr_ty = cg.context.ptr_type(inkwell::AddressSpace::default());
            let buf = cg.builder.build_array_alloca(i8_ty, i64_ty.const_int(64, false), "str_buf").unwrap();
            if cg.module.get_function("sprintf").is_none() {
                let ft = i64_ty.fn_type(&[ptr_ty.into(), ptr_ty.into()], true);
                cg.module.add_function("sprintf", ft, None);
            }
            let fmt = cg.builder.build_global_string_ptr("%g", "floatfmt").unwrap();
            cg.builder.build_call(cg.module.get_function("sprintf").unwrap(), &[buf.into(), fmt.as_pointer_value().into(), f.into()], "float_to_str").unwrap();
            return Ok(buf.into());
        }
        _ => {}
    }

    let i = val.into_int_value();
    let cf = cg.function.unwrap();
    let i64_zero = i64_ty.const_zero();
    let buf = cg.builder.build_array_alloca(i8_ty, i64_ty.const_int(24, false), "str_buf").unwrap();
    let is_neg = cg.builder.build_int_compare(inkwell::IntPredicate::SLT, i, i64_zero, "is_neg").unwrap();
    let neg_i = cg.builder.build_int_neg(i, "neg_i").unwrap();
    let abs_i = cg.builder.build_select(is_neg, neg_i, i, "abs_i").unwrap();
    let pos_alloca = cg.create_entry_alloca(i64_ty.into(), "str_pos");
    let cur_alloca = cg.create_entry_alloca(i64_ty.into(), "str_cur");
    cg.builder.build_store(cur_alloca, abs_i).unwrap();
    cg.builder.build_store(pos_alloca, i64_ty.const_int(22, false)).unwrap();
    let null_at = unsafe { cg.builder.build_gep(i8_ty, buf, &[i64_ty.const_int(22, false)], "null_at").unwrap() };
    cg.builder.build_store(null_at, i8_ty.const_zero()).unwrap();
    let cond_bb = cg.context.append_basic_block(cf, "st_cond");
    let body_bb = cg.context.append_basic_block(cf, "st_body");
    let after_bb = cg.context.append_basic_block(cf, "st_aft");
    cg.builder.build_unconditional_branch(cond_bb).unwrap();
    cg.builder.position_at_end(cond_bb);
    let cv = cg.builder.build_load(i64_ty, cur_alloca, "cv").unwrap().into_int_value();
    let is_done = cg.builder.build_int_compare(inkwell::IntPredicate::EQ, cv, i64_zero, "isd").unwrap();
    cg.builder.build_conditional_branch(is_done, after_bb, body_bb).unwrap();
    cg.builder.position_at_end(body_bb);
    cg.loop_stack.push((cond_bb, after_bb));
    let cv2 = cg.builder.build_load(i64_ty, cur_alloca, "cv2").unwrap().into_int_value();
    let dig = cg.builder.build_int_signed_rem(cv2, i64_ty.const_int(10, false), "dig").unwrap();
    let ch = cg.builder.build_int_add(dig, i64_ty.const_int(48, false), "ch").unwrap();
    let chi = cg.builder.build_int_truncate(ch, i8_ty, "chi").unwrap();
    let p1 = cg.builder.build_load(i64_ty, pos_alloca, "p1").unwrap().into_int_value();
    let p2 = cg.builder.build_int_sub(p1, i64_ty.const_int(1, false), "p2").unwrap();
    cg.builder.build_store(pos_alloca, p2).unwrap();
    let gp = unsafe { cg.builder.build_gep(i8_ty, buf, &[p2], "gp").unwrap() };
    cg.builder.build_store(gp, chi).unwrap();
    let dv = cg.builder.build_int_signed_div(cv2, i64_ty.const_int(10, false), "dv").unwrap();
    cg.builder.build_store(cur_alloca, dv).unwrap();
    cg.builder.build_unconditional_branch(cond_bb).unwrap();
    cg.loop_stack.pop();
    cg.builder.position_at_end(after_bb);
    let iz = cg.builder.build_int_compare(inkwell::IntPredicate::EQ, abs_i.into_int_value(), i64_zero, "iz").unwrap();
    let zb = cg.context.append_basic_block(cf, "st_zb");
    let sk = cg.context.append_basic_block(cf, "st_sk");
    cg.builder.build_conditional_branch(iz, zb, sk).unwrap();
    cg.builder.position_at_end(zb);
    let pz = cg.builder.build_load(i64_ty, pos_alloca, "pz").unwrap().into_int_value();
    let pz2 = cg.builder.build_int_sub(pz, i64_ty.const_int(1, false), "pz2").unwrap();
    cg.builder.build_store(pos_alloca, pz2).unwrap();
    let gz = unsafe { cg.builder.build_gep(i8_ty, buf, &[pz2], "gz").unwrap() };
    cg.builder.build_store(gz, i8_ty.const_int(48, false)).unwrap();
    cg.builder.build_unconditional_branch(sk).unwrap();
    cg.builder.position_at_end(sk);
    let nb = cg.context.append_basic_block(cf, "st_nb");
    let nn = cg.context.append_basic_block(cf, "st_nn");
    cg.builder.build_conditional_branch(is_neg, nb, nn).unwrap();
    cg.builder.position_at_end(nb);
    let pn = cg.builder.build_load(i64_ty, pos_alloca, "pn").unwrap().into_int_value();
    let pn2 = cg.builder.build_int_sub(pn, i64_ty.const_int(1, false), "pn2").unwrap();
    cg.builder.build_store(pos_alloca, pn2).unwrap();
    let gn = unsafe { cg.builder.build_gep(i8_ty, buf, &[pn2], "gn").unwrap() };
    cg.builder.build_store(gn, i8_ty.const_int(45, false)).unwrap();
    cg.builder.build_unconditional_branch(nn).unwrap();
    cg.builder.position_at_end(nn);
    let fp = cg.builder.build_load(i64_ty, pos_alloca, "fp").unwrap().into_int_value();
    let sp = unsafe { cg.builder.build_gep(i8_ty, buf, &[fp], "sp").unwrap() };
    Ok(sp.into())
}
