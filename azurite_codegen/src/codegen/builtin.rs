use azurite_parser::ast::*;
use inkwell::values::{BasicMetadataValueEnum, BasicValueEnum, FunctionValue};
use crate::codegen::CodeGen;

/// Returns true if the expression is known to produce a boolean value
fn is_bool_expr(expr: &Expr) -> bool {
    match expr {
        Expr::Bool(_) => true,
        Expr::Binary { op, .. } => matches!(op, BinOp::Eq | BinOp::Neq | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge | BinOp::And | BinOp::Or | BinOp::Is),
        Expr::Unary { op: UnOp::Not, .. } => true,
        Expr::Call { callee, .. } => {
            if let Expr::Ident(i) = callee.as_ref() {
                i.name == "has"
            } else { false }
        }
        _ => false,
    }
}

pub fn compile_print<'ctx>(cg: &mut CodeGen<'ctx>, args: &[Expr]) -> Result<BasicValueEnum<'ctx>, azurite_errors::AzError> {
    let printf = get_or_declare_printf(cg);
    let i64_ty = cg.context.i64_type();

    for arg_expr in args {
        // Check if this is an `any` variable
        if let Expr::Ident(ident) = arg_expr {
            if let Some(tag_ptr) = cg.any_tags.get(&ident.name) {
                // Load the tag and dispatch
                let tag = cg.builder.build_load(i64_ty, *tag_ptr, "tag").unwrap().into_int_value();
                let val_ptr = cg.variables.get(&ident.name).unwrap().0;
                let v_gep = unsafe { cg.builder.build_gep(i64_ty, val_ptr, &[i64_ty.const_zero()], "v").unwrap() };
                let val_i64 = cg.builder.build_load(i64_ty, v_gep, "v").unwrap();

                // Create dispatch blocks for tag 0=int, 1=float, 2=string, 3=bool
                let cf = cg.function.unwrap();
                let int_bb = cg.context.append_basic_block(cf, "any_int");
                let float_bb = cg.context.append_basic_block(cf, "any_float");
                let str_bb = cg.context.append_basic_block(cf, "any_str");
                let bool_bb = cg.context.append_basic_block(cf, "any_bool");
                let merge_bb = cg.context.append_basic_block(cf, "any_end");

                let is_float = cg.builder.build_int_compare(inkwell::IntPredicate::EQ, tag, i64_ty.const_int(1, false), "tf").unwrap();
                let is_str = cg.builder.build_int_compare(inkwell::IntPredicate::EQ, tag, i64_ty.const_int(2, false), "ts").unwrap();
                let is_bool = cg.builder.build_int_compare(inkwell::IntPredicate::EQ, tag, i64_ty.const_int(3, false), "tb").unwrap();
                let next1 = cg.context.append_basic_block(cf, "any_c1");
                cg.builder.build_conditional_branch(is_float, float_bb, next1).unwrap();
                cg.builder.position_at_end(next1);
                let next2 = cg.context.append_basic_block(cf, "any_c2");
                cg.builder.build_conditional_branch(is_str, str_bb, next2).unwrap();
                cg.builder.position_at_end(next2);
                cg.builder.build_conditional_branch(is_bool, bool_bb, int_bb).unwrap(); // if not bool, it's int

                // int branch
                cg.builder.position_at_end(int_bb);
                let int_fmt = cg.builder.build_global_string_ptr("%d", "if").unwrap();
                cg.builder.build_call(printf, &[int_fmt.as_pointer_value().into(), val_i64.into()], "pi").unwrap();
                cg.builder.build_unconditional_branch(merge_bb).unwrap();

                // float branch
                cg.builder.position_at_end(float_bb);
                let f = cg.builder.build_signed_int_to_float(val_i64.into_int_value(), cg.context.f64_type(), "i2f").unwrap();
                let f_fmt = cg.builder.build_global_string_ptr("%g", "ff").unwrap();
                cg.builder.build_call(printf, &[f_fmt.as_pointer_value().into(), f.into()], "pf").unwrap();
                cg.builder.build_unconditional_branch(merge_bb).unwrap();

                // string branch
                cg.builder.position_at_end(str_bb);
                let p = cg.builder.build_int_to_ptr(val_i64.into_int_value(), cg.context.ptr_type(inkwell::AddressSpace::default()), "i2p").unwrap();
                let s_fmt = cg.builder.build_global_string_ptr("%s", "sf").unwrap();
                cg.builder.build_call(printf, &[s_fmt.as_pointer_value().into(), p.into()], "ps").unwrap();
                cg.builder.build_unconditional_branch(merge_bb).unwrap();

                // bool branch
                cg.builder.position_at_end(bool_bb);
                let zero = i64_ty.const_zero();
                let cmp = cg.builder.build_int_compare(inkwell::IntPredicate::NE, val_i64.into_int_value(), zero, "bc").unwrap();
                let t_str = cg.builder.build_global_string_ptr("true", "ts").unwrap();
                let f_str = cg.builder.build_global_string_ptr("false", "fs").unwrap();
                let sel = cg.builder.build_select(cmp, t_str.as_pointer_value(), f_str.as_pointer_value(), "bs").unwrap();
                let b_fmt = cg.builder.build_global_string_ptr("%s", "bf").unwrap();
                cg.builder.build_call(printf, &[b_fmt.as_pointer_value().into(), sel.into()], "pb").unwrap();
                cg.builder.build_unconditional_branch(merge_bb).unwrap();

                cg.builder.position_at_end(merge_bb);
                continue;
            }
        }

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
