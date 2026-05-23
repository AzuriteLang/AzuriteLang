use azurite_parser::ast::*;
use inkwell::values::BasicValueEnum;
use crate::codegen::CodeGen;

pub fn compile_sqrt<'ctx>(cg: &mut CodeGen<'ctx>, args: &[Expr]) -> Result<BasicValueEnum<'ctx>, azurite_errors::AzError> {
    let val = cg.compile_expr(&args[0])?;
    let f = val.into_float_value();
    let f64_ty = cg.context.f64_type();
    let ft = f64_ty.fn_type(&[f64_ty.into()], false);
    let intrinsic = cg.module.add_function("llvm.sqrt.f64", ft, None);
    let result = cg.builder.build_call(intrinsic, &[f.into()], "sqrt").unwrap();
    Ok(match result.try_as_basic_value() { inkwell::values::ValueKind::Basic(bv) => bv, _ => cg.context.f64_type().const_float(0.0).into() })
}

pub fn compile_abs<'ctx>(cg: &mut CodeGen<'ctx>, args: &[Expr]) -> Result<BasicValueEnum<'ctx>, azurite_errors::AzError> {
    let val = cg.compile_expr(&args[0])?;
    let i = val.into_int_value();
    let zero = cg.context.i64_type().const_zero();
    let neg = cg.builder.build_int_neg(i, "neg").unwrap();
    let cmp = cg.builder.build_int_compare(inkwell::IntPredicate::SLT, i, zero, "cmp").unwrap();
    Ok(cg.builder.build_select(cmp, neg, i, "abs").unwrap())
}

pub fn compile_math1<'ctx>(cg: &mut CodeGen<'ctx>, name: &str, args: &[Expr]) -> Result<BasicValueEnum<'ctx>, azurite_errors::AzError> {
    let val = cg.compile_expr(&args[0])?;
    let f = val.into_float_value();
    let f64_ty = cg.context.f64_type();
    let ft = f64_ty.fn_type(&[f64_ty.into()], false);
    let func = match cg.module.get_function(name) {
        Some(f) => f,
        None => cg.module.add_function(name, ft, None),
    };
    let result = cg.builder.build_call(func, &[f.into()], name).unwrap();
    Ok(match result.try_as_basic_value() { inkwell::values::ValueKind::Basic(bv) => bv, _ => f64_ty.const_float(0.0).into() })
}

pub fn compile_math2<'ctx>(cg: &mut CodeGen<'ctx>, name: &str, args: &[Expr]) -> Result<BasicValueEnum<'ctx>, azurite_errors::AzError> {
    let a = cg.compile_expr(&args[0])?;
    let b = cg.compile_expr(&args[1])?;
    let fa = a.into_float_value();
    let fb = b.into_float_value();
    let f64_ty = cg.context.f64_type();
    let ft = f64_ty.fn_type(&[f64_ty.into(), f64_ty.into()], false);
    let func = match cg.module.get_function(name) {
        Some(f) => f,
        None => cg.module.add_function(name, ft, None),
    };
    let result = cg.builder.build_call(func, &[fa.into(), fb.into()], name).unwrap();
    Ok(match result.try_as_basic_value() { inkwell::values::ValueKind::Basic(bv) => bv, _ => f64_ty.const_float(0.0).into() })
}

pub fn compile_rand<'ctx>(cg: &mut CodeGen<'ctx>, _args: &[Expr]) -> Result<BasicValueEnum<'ctx>, azurite_errors::AzError> {
    let i32_ty = cg.context.i32_type();
    let ft = i32_ty.fn_type(&[], false);
    let func = match cg.module.get_function("rand") {
        Some(f) => f,
        None => cg.module.add_function("rand", ft, None),
    };
    let result = cg.builder.build_call(func, &[], "rand").unwrap();
    let val = match result.try_as_basic_value() {
        inkwell::values::ValueKind::Basic(bv) => bv.into_int_value(),
        _ => i32_ty.const_zero(),
    };
    Ok(cg.builder.build_int_z_extend(val, cg.context.i64_type(), "rand_ext").unwrap().into())
}

pub fn compile_srand<'ctx>(cg: &mut CodeGen<'ctx>, args: &[Expr]) -> Result<BasicValueEnum<'ctx>, azurite_errors::AzError> {
    let val = cg.compile_expr(&args[0])?.into_int_value();
    let seed = cg.builder.build_int_truncate(val, cg.context.i32_type(), "seed_trunc").unwrap();
    let ft = cg.context.void_type().fn_type(&[cg.context.i32_type().into()], false);
    let func = match cg.module.get_function("srand") {
        Some(f) => f,
        None => cg.module.add_function("srand", ft, None),
    };
    cg.builder.build_call(func, &[seed.into()], "srand").unwrap();
    Ok(cg.context.i64_type().const_zero().into())
}
