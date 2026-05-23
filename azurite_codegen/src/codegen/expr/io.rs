use azurite_errors::{AzError, ErrorKind};
use azurite_lexer::Span;
use azurite_parser::ast::*;
use inkwell::values::BasicValueEnum;
use crate::codegen::CodeGen;

pub fn dispatch<'ctx>(cg: &mut CodeGen<'ctx>, name: &str, args: &[Expr]) -> Result<BasicValueEnum<'ctx>, AzError> {
    match name {
        "read" => compile_read(cg),
        "input" => compile_input(cg, args),
        "exit" => compile_exit(cg, args),
        "getenv" => compile_getenv(cg, args),
        "system" => compile_system(cg, args),
        "pid" => compile_pid(cg, args),
        "cwd" => compile_cwd(cg, args),
        _ => Err(AzError::new(ErrorKind::Semantic, Span::new(0, 0, 0, 0), format!("unknown io builtin '{}'", name))),
    }
}

fn compile_read<'ctx>(cg: &mut CodeGen<'ctx>) -> Result<BasicValueEnum<'ctx>, AzError> {
    let i64_ty = cg.context.i64_type();
    let i8_ty = cg.context.i8_type();
    let buf = cg.builder.build_array_alloca(i8_ty, i64_ty.const_int(1024, false), "read_buf").unwrap();
    let i_ptr = cg.create_entry_alloca(i64_ty.into(), "__read_i");
    cg.builder.build_store(i_ptr, i64_ty.const_zero()).unwrap();
    let cf = cg.function.unwrap();
    let cond_bb = cg.context.append_basic_block(cf, "rd_cond");
    let body_bb = cg.context.append_basic_block(cf, "rd_body");
    let done_bb = cg.context.append_basic_block(cf, "rd_done");
    let getchar_ty = cg.context.i32_type().fn_type(&[], false);
    let getchar = match cg.module.get_function("getchar") {
        Some(f) => f,
        None => cg.module.add_function("getchar", getchar_ty, None),
    };
    cg.builder.build_unconditional_branch(cond_bb).unwrap();
    cg.builder.position_at_end(cond_bb);
    let i_val = cg.builder.build_load(i64_ty, i_ptr, "i").unwrap().into_int_value();
    let cmp = cg.builder.build_int_compare(inkwell::IntPredicate::SLT, i_val, i64_ty.const_int(1023, false), "read_cmp").unwrap();
    cg.builder.build_conditional_branch(cmp, body_bb, done_bb).unwrap();
    cg.builder.position_at_end(body_bb);
    let c = cg.builder.build_call(getchar, &[], "getchar").unwrap();
    let c_val = c.try_as_basic_value().unwrap_basic().into_int_value();
    let c_i64 = cg.builder.build_int_z_extend(c_val, i64_ty, "ext").unwrap();
    let is_nl = cg.builder.build_int_compare(inkwell::IntPredicate::EQ, c_val, i64_ty.const_int(10, false), "is_nl").unwrap();
    let is_eof = cg.builder.build_int_compare(inkwell::IntPredicate::EQ, c_val, i64_ty.const_int(0xFFFFFFFF, false), "is_eof").unwrap();
    let should_stop = cg.builder.build_or(is_nl, is_eof, "should_stop").unwrap();
    let should_stop_bool = cg.builder.build_int_compare(inkwell::IntPredicate::NE, should_stop, cg.context.bool_type().const_zero(), "stop_chk").unwrap();
    let gep = unsafe { cg.builder.build_gep(i8_ty, buf, &[i_val], "gep").unwrap() };
    let c_i8 = cg.builder.build_int_truncate(c_i64, i8_ty, "trunc").unwrap();
    cg.builder.build_store(gep, c_i8).unwrap();
    let next_i = cg.builder.build_int_add(i_val, i64_ty.const_int(1, false), "next").unwrap();
    cg.builder.build_store(i_ptr, next_i).unwrap();
    cg.builder.build_conditional_branch(should_stop_bool, done_bb, cond_bb).unwrap();
    cg.builder.position_at_end(done_bb);
    let i_final = cg.builder.build_load(i64_ty, i_ptr, "i_final").unwrap().into_int_value();
    let null_gep = unsafe { cg.builder.build_gep(i8_ty, buf, &[i_final], "null_gep").unwrap() };
    cg.builder.build_store(null_gep, i8_ty.const_zero()).unwrap();
    Ok(buf.into())
}

fn compile_input<'ctx>(cg: &mut CodeGen<'ctx>, args: &[Expr]) -> Result<BasicValueEnum<'ctx>, AzError> {
    let prompt = cg.compile_expr(&args[0])?;
    let printf = super::super::builtin::get_or_declare_printf(cg);
    let fmt = cg.builder.build_global_string_ptr("%s", "promptfmt").unwrap();
    cg.builder.build_call(printf, &[fmt.as_pointer_value().into(), prompt.into()], "printprompt").unwrap();
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

fn compile_getenv<'ctx>(cg: &mut CodeGen<'ctx>, _args: &[Expr]) -> Result<BasicValueEnum<'ctx>, AzError> {
    let empty = cg.builder.build_global_string_ptr("", "empty_env").unwrap();
    Ok(empty.as_pointer_value().into())
}

fn compile_system<'ctx>(cg: &mut CodeGen<'ctx>, args: &[Expr]) -> Result<BasicValueEnum<'ctx>, AzError> {
    let cmd = cg.compile_expr(&args[0])?;
    let i64_ty = cg.context.i64_type();
    if cg.module.get_function("system").is_none() {
        let ft = i64_ty.fn_type(&[cg.context.ptr_type(inkwell::AddressSpace::default()).into()], false);
        cg.module.add_function("system", ft, None);
    }
    let result = cg.builder.build_call(cg.module.get_function("system").unwrap(), &[cmd.into()], "system_call").unwrap();
    Ok(result.try_as_basic_value().unwrap_basic())
}

fn compile_pid<'ctx>(cg: &mut CodeGen<'ctx>, _args: &[Expr]) -> Result<BasicValueEnum<'ctx>, AzError> {
    let i64_ty = cg.context.i64_type();
    for name in &["_getpid" as &str, "getpid"] {
        if cg.module.get_function(name).is_none() {
            let ft = i64_ty.fn_type(&[], false);
            cg.module.add_function(name, ft, None);
        }
    }
    let pname = if cg.module.get_function("_getpid").is_some() { "_getpid" } else { "getpid" };
    let result = cg.builder.build_call(cg.module.get_function(pname).unwrap(), &[], "pid_call").unwrap();
    Ok(result.try_as_basic_value().unwrap_basic())
}

fn compile_cwd<'ctx>(cg: &mut CodeGen<'ctx>, _args: &[Expr]) -> Result<BasicValueEnum<'ctx>, AzError> {
    let ptr_ty = cg.context.ptr_type(inkwell::AddressSpace::default());
    let i64_ty = cg.context.i64_type();
    let buf = cg.builder.build_array_alloca(cg.context.i8_type(), i64_ty.const_int(1024, false), "cwd_buf").unwrap();
    for name in &["_getcwd" as &str, "getcwd"] {
        if cg.module.get_function(name).is_none() {
            let ft = ptr_ty.fn_type(&[ptr_ty.into(), i64_ty.into()], false);
            cg.module.add_function(name, ft, None);
        }
    }
    let cname = if cg.module.get_function("_getcwd").is_some() { "_getcwd" } else { "getcwd" };
    cg.builder.build_call(cg.module.get_function(cname).unwrap(), &[buf.into(), i64_ty.const_int(1024, false).into()], "cwd_call").unwrap();
    Ok(buf.into())
}
