use azurite_errors::{AzError, ErrorKind};
use azurite_lexer::Span;
use azurite_parser::ast::*;
use inkwell::values::{BasicMetadataValueEnum, BasicValueEnum, IntValue};
use inkwell::IntPredicate;
use crate::codegen::CodeGen;

pub fn compile_call<'ctx>(cg: &mut CodeGen<'ctx>, expr: &Expr) -> Result<BasicValueEnum<'ctx>, AzError> {
    match expr {
        Expr::Call { callee, args } => {
            let callee_name = match callee.as_ref() {
                Expr::Ident(i) => i.name.clone(),
                _ => return Err(AzError::new(ErrorKind::Semantic, Span::new(0, 0, 0, 0), "invalid callee")),
            };
            match callee_name.as_str() {
                "print" => return super::super::builtin::compile_print(cg, args),
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
            let compiled = args.iter().map(|a| cg.compile_expr(a)).collect::<Result<Vec<_>, _>>()?;
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
        Expr::MethodCall { obj, method, args } => {
            // Constructor call: ClassName.new(...)
            if method == "new" {
                if let Expr::Ident(ident) = obj {
                    let fn_name = format!("{}_{}", ident.name, method);
                    if let Some(f) = cg.module.get_function(&fn_name) {
                        let compiled = args.iter().map(|a| cg.compile_expr(a)).collect::<Result<Vec<_>, _>>()?;
                        let meta: Vec<BasicMetadataValueEnum> = compiled.iter().map(|a| (*a).into()).collect();
                        let result = cg.builder.build_call(f, &meta, "calltmp").unwrap();
                        return Ok(match result.try_as_basic_value() {
                            inkwell::values::ValueKind::Basic(bv) => bv,
                            _ => cg.context.i64_type().const_zero().into(),
                        });
                    }
                }
            }
            // Instance method call: instance.method(args)
            let obj_val = cg.compile_expr(obj)?;
            let compiled = args.iter().map(|a| cg.compile_expr(a)).collect::<Result<Vec<_>, _>>()?;
            for (class_name, info) in &cg.struct_types {
                if info.methods.iter().any(|m| m == method) {
                    let fn_name = format!("{}_{}", class_name, method);
                    if let Some(f) = cg.module.get_function(&fn_name) {
                        let mut meta: Vec<BasicMetadataValueEnum> = vec![obj_val.into()];
                        for a in &compiled { meta.push((*a).into()); }
                        let result = cg.builder.build_call(f, &meta, "calltmp").unwrap();
                        return Ok(match result.try_as_basic_value() {
                            inkwell::values::ValueKind::Basic(bv) => bv,
                            _ => cg.context.i64_type().const_zero().into(),
                        });
                    }
                }
            }
            Ok(cg.context.i64_type().const_zero().into())
        }
        _ => unreachable!(),
    }
}

// --- Built-in implementations ---

fn compile_sqrt<'ctx>(cg: &mut CodeGen<'ctx>, args: &[Expr]) -> Result<BasicValueEnum<'ctx>, AzError> {
    let val = cg.compile_expr(&args[0])?;
    let f = val.into_float_value();
    let f64_ty = cg.context.f64_type();
    let ft = f64_ty.fn_type(&[f64_ty.into()], false);
    let intrinsic = cg.module.add_function("llvm.sqrt.f64", ft, None);
    let result = cg.builder.build_call(intrinsic, &[f.into()], "sqrt").unwrap();
    Ok(match result.try_as_basic_value() { inkwell::values::ValueKind::Basic(bv) => bv, _ => cg.context.f64_type().const_float(0.0).into() })
}

fn compile_abs<'ctx>(cg: &mut CodeGen<'ctx>, args: &[Expr]) -> Result<BasicValueEnum<'ctx>, AzError> {
    let val = cg.compile_expr(&args[0])?;
    let i = val.into_int_value();
    let zero = cg.context.i64_type().const_zero();
    let neg = cg.builder.build_int_neg(i, "neg").unwrap();
    let cmp = cg.builder.build_int_compare(IntPredicate::SLT, i, zero, "cmp").unwrap();
    Ok(cg.builder.build_select(cmp, neg, i, "abs").unwrap())
}

fn compile_len<'ctx>(cg: &mut CodeGen<'ctx>, args: &[Expr]) -> Result<BasicValueEnum<'ctx>, AzError> {
    let val = cg.compile_expr(&args[0])?;
    let ptr = val.into_pointer_value();
    let ptr_ty = cg.context.ptr_type(inkwell::AddressSpace::default());
    let strlen_ty = cg.context.i64_type().fn_type(&[ptr_ty.into()], false);
    cg.module.add_function("strlen", strlen_ty, None);
    let len = cg.builder.build_call(cg.module.get_function("strlen").unwrap(), &[ptr.into()], "len").unwrap();
    Ok(match len.try_as_basic_value() { inkwell::values::ValueKind::Basic(bv) => bv, _ => cg.context.i64_type().const_zero().into() })
}

fn compile_read<'ctx>(cg: &mut CodeGen<'ctx>) -> Result<BasicValueEnum<'ctx>, AzError> {
    let _buf = cg.builder.build_alloca(cg.context.i64_type(), "buf").unwrap();
    let ptr_ty = cg.context.ptr_type(inkwell::AddressSpace::default());
    let fgets_ty = ptr_ty.fn_type(&[ptr_ty.into(), cg.context.i64_type().into(), ptr_ty.into()], false);
    cg.module.add_function("fgets", fgets_ty, None);
    let empty = cg.builder.build_global_string_ptr("", "empty").unwrap();
    Ok(empty.as_pointer_value().into())
}

fn compile_input<'ctx>(cg: &mut CodeGen<'ctx>, args: &[Expr]) -> Result<BasicValueEnum<'ctx>, AzError> {
    let _ = cg.compile_expr(&args[0])?;
    compile_read(cg)
}

fn compile_exit<'ctx>(cg: &mut CodeGen<'ctx>, args: &[Expr]) -> Result<BasicValueEnum<'ctx>, AzError> {
    let val = cg.compile_expr(&args[0])?;
    let i32_val = cg.builder.build_int_truncate(val.into_int_value(), cg.context.i32_type(), "ec").unwrap();
    let exit_ty = cg.context.void_type().fn_type(&[cg.context.i32_type().into()], false);
    cg.module.add_function("exit", exit_ty, None);
    cg.builder.build_call(cg.module.get_function("exit").unwrap(), &[i32_val.into()], "exit").unwrap();
    Ok(cg.context.i64_type().const_zero().into())
}

fn compile_int_cast<'ctx>(cg: &mut CodeGen<'ctx>, args: &[Expr]) -> Result<BasicValueEnum<'ctx>, AzError> {
    let val = cg.compile_expr(&args[0])?;
    Ok(cg.builder.build_float_to_signed_int(val.into_float_value(), cg.context.i64_type(), "f2i").unwrap().into())
}

fn compile_float_cast<'ctx>(cg: &mut CodeGen<'ctx>, args: &[Expr]) -> Result<BasicValueEnum<'ctx>, AzError> {
    let val = cg.compile_expr(&args[0])?;
    Ok(cg.builder.build_signed_int_to_float(val.into_int_value(), cg.context.f64_type(), "i2f").unwrap().into())
}
