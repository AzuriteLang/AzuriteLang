use azurite_parser::ast::*;
use inkwell::values::{BasicMetadataValueEnum, BasicValueEnum, FunctionValue};
use crate::codegen::CodeGen;

/// Returns true if the expression is known to produce a boolean value
fn is_bool_expr(expr: &Expr) -> bool {
    match expr {
        Expr::Bool(_) => true,
        Expr::Binary { op, .. } => matches!(op, BinOp::Eq | BinOp::Neq | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge | BinOp::And | BinOp::Or),
        Expr::Unary { op: UnOp::Not, .. } => true,
        Expr::Call { callee, .. } => {
            if let Expr::Ident(i) = callee.as_ref() {
                matches!(i.name.as_str(), "is_prime" | "has_xxx" /* add known bool funcs */)
            } else { false }
        }
        _ => false,
    }
}

pub fn compile_print<'ctx>(cg: &mut CodeGen<'ctx>, args: &[Expr]) -> Result<BasicValueEnum<'ctx>, azurite_errors::AzError> {
    let printf = get_or_declare_printf(cg);

    for arg_expr in args {
        if is_bool_expr(arg_expr) {
            let val = cg.compile_expr(arg_expr)?;
            let zero = cg.context.i64_type().const_zero();
            let iv = val.into_int_value();
            let cmp = cg.builder.build_int_compare(inkwell::IntPredicate::NE, iv, zero, "boolchk").unwrap();
            let true_str = cg.builder.build_global_string_ptr("true", "ts").unwrap();
            let false_str = cg.builder.build_global_string_ptr("false", "fs").unwrap();
            let sel = cg.builder.build_select(cmp, true_str.as_pointer_value(), false_str.as_pointer_value(), "boolsel").unwrap();
            let fmt = cg.builder.build_global_string_ptr("%s", "bf").unwrap();
            cg.builder.build_call(printf, &[fmt.as_pointer_value().into(), sel.into()], "pt").unwrap();
        } else {
            let val = cg.compile_expr(arg_expr)?;
            let (fmt, data) = get_print_format(cg, &val);
            let mut printf_args: Vec<BasicMetadataValueEnum> = vec![fmt.into()];
            if let Some(d) = data { printf_args.push(d.into()); }
            cg.builder.build_call(printf, &printf_args, "printtmp").unwrap();
        }
    }

    let nl = cg.context.i8_type().const_int(b'\n' as u64, false);
    let putchar = get_or_declare_putchar(cg);
    cg.builder.build_call(putchar, &[nl.into()], "nl").unwrap();

    Ok(cg.context.i64_type().const_zero().into())
}

fn get_print_format<'ctx>(cg: &CodeGen<'ctx>, val: &BasicValueEnum<'ctx>) -> (inkwell::values::PointerValue<'ctx>, Option<BasicValueEnum<'ctx>>) {
    match val {
        BasicValueEnum::IntValue(_) => {
            let g = cg.builder.build_global_string_ptr("%d", "intfmt").unwrap();
            (g.as_pointer_value(), Some(*val))
        }
        BasicValueEnum::FloatValue(_) => {
            let g = cg.builder.build_global_string_ptr("%g", "floatfmt").unwrap();
            (g.as_pointer_value(), Some(*val))
        }
        BasicValueEnum::PointerValue(_) => {
            let g = cg.builder.build_global_string_ptr("%s", "strfmt").unwrap();
            (g.as_pointer_value(), Some(*val))
        }
        _ => {
            let g = cg.builder.build_global_string_ptr("%ld", "defaultfmt").unwrap();
            (g.as_pointer_value(), Some(*val))
        }
    }
}

pub fn get_or_declare_printf<'ctx>(cg: &mut CodeGen<'ctx>) -> FunctionValue<'ctx> {
    if let Some(pf) = cg.printf { return pf; }
    let i64 = cg.context.i64_type();
    let ptr = cg.context.ptr_type(inkwell::AddressSpace::default());
    let ft = i64.fn_type(&[ptr.into()], true);
    let pf = cg.module.add_function("printf", ft, None);
    cg.printf = Some(pf);
    pf
}

pub fn get_or_declare_putchar<'ctx>(cg: &mut CodeGen<'ctx>) -> FunctionValue<'ctx> {
    if let Some(pc) = cg.putchar { return pc; }
    let i32 = cg.context.i32_type();
    let ft = i32.fn_type(&[i32.into()], false);
    let pc = cg.module.add_function("putchar", ft, None);
    cg.putchar = Some(pc);
    pc
}
