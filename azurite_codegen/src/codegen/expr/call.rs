use azurite_errors::{AzError, ErrorKind};
use azurite_lexer::Span;
use azurite_parser::ast::*;
use inkwell::values::{BasicMetadataValueEnum, BasicValueEnum};
use inkwell::IntPredicate;
use crate::codegen::CodeGen;

pub fn compile_call<'ctx>(cg: &mut CodeGen<'ctx>, expr: &Expr) -> Result<BasicValueEnum<'ctx>, AzError> {
    match expr {
        Expr::Call { callee, args } => {
            let callee_name = match callee.as_ref() {
                Expr::Ident(i) => i.name.clone(),
                _ => return Err(AzError::new(ErrorKind::Semantic, expr.span(), "invalid callee")),
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
                "char_at" => return compile_char_at(cg, args),
                "chr" => return compile_chr(cg, args),
                "sin" => return compile_math1(cg, "sin", args),
                "cos" => return compile_math1(cg, "cos", args),
                "tan" => return compile_math1(cg, "tan", args),
                "log" => return compile_math1(cg, "log", args),
                "log10" => return compile_math1(cg, "log10", args),
                "floor" => return compile_math1(cg, "floor", args),
                "ceil" => return compile_math1(cg, "ceil", args),
                "pow" => return compile_math2(cg, "pow", args),
                "asin" => return compile_math1(cg, "asin", args),
                "acos" => return compile_math1(cg, "acos", args),
                "atan" => return compile_math1(cg, "atan", args),
                "atan2" => return compile_math2(cg, "atan2", args),
                "sinh" => return compile_math1(cg, "sinh", args),
                "cosh" => return compile_math1(cg, "cosh", args),
                "tanh" => return compile_math1(cg, "tanh", args),
                "exp" => return compile_math1(cg, "exp", args),
                "expm1" => return compile_math1(cg, "expm1", args),
                "log2" => return compile_math1(cg, "log2", args),
                "hypot" => return compile_math2(cg, "hypot", args),
                "fmod" => return compile_math2(cg, "fmod", args),
                "copysign" => return compile_math2(cg, "copysign", args),
                "rand" => return compile_rand(cg, args),
                "srand" => return compile_srand(cg, args),
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
                Err(AzError::new(ErrorKind::Semantic, expr.span(), format!("undefined '{}'", callee_name)))
            }
        }
        Expr::MethodCall { obj, method, args, null_safe } => {
            // Constructor call: ClassName.new(...)
            if method == "new" {
                if let Expr::Ident(ident) = obj.as_ref() {
                    let fn_name = format!("{}_{}", ident.name, method);
                    // Check if this is a generic class that needs instantiation
                    if cg.module.get_function(&fn_name).is_none() {
                        if let Some((tp, fields, methods)) = cg.generic_classes.get(&ident.name).cloned() {
                            // Determine concrete types from all args
                            let concrete_types: Vec<String> = if args.is_empty() { vec!["void".to_string()] } else {
                                args.iter().map(|a| match a {
                                    Expr::Int(_) => "int",
                                    Expr::Float(_) => "float",
                                    Expr::String(_) => "string",
                                    Expr::Bool(_) => "bool",
                                    _ => "any",
                                }.to_string()).collect()
                            };
                            let concrete_suffix = concrete_types.join("_");
                            let concrete_name = format!("{}_{}", ident.name, concrete_suffix);
                            // Add auto-generated 'new' method if not present
                            let has_new = methods.iter().any(|m| matches!(m, Stmt::Func { name: mn, .. } if mn.name == "new"));
                            let concrete_methods: Vec<Stmt> = if has_new {
                                methods.clone()
                            } else {
                                let mut m = methods.clone();
                                let new_params: Vec<Param> = fields.iter().map(|f| Param {
                                    name: f.name.clone(),
                                    type_annotation: Some(subst_type_multi(&f.type_, &tp, &concrete_types)),
                                }).collect();
                                m.push(Stmt::Func {
                                    name: Ident { name: "new".to_string(), span: Span::new(0, 0, 1, 1) },
                                    params: new_params,
                                    return_type: None,
                                    body: Box::new(Expr::Block(vec![])),
                                });
                                m
                            };
                            // Substitute types in fields
                            let concrete_fields: Vec<ClassField> = fields.iter().map(|f| ClassField {
                                name: f.name.clone(),
                                type_: subst_type_multi(&f.type_, &tp, &concrete_types),
                            }).collect();
                            // Compile the concrete class with concrete methods
                            let saved_fn = cg.function;
                            let saved_self_ptr = cg.self_ptr.take();
                            let saved_class = cg.current_class.take();
                            let saved_vars = std::mem::take(&mut cg.variables);
                            let saved_block = cg.builder.get_insert_block();
                            let concrete_ident = Ident { name: concrete_name.clone(), span: azurite_lexer::Span::new(0, 0, 0, 0) };
                            super::super::class::compile_class(cg, &concrete_ident, &concrete_fields, &concrete_methods, &None)?;
                            cg.function = saved_fn;
                            cg.self_ptr = saved_self_ptr;
                            cg.current_class = saved_class;
                            cg.variables = saved_vars;
                            if let Some(bb) = saved_block {
                                cg.builder.position_at_end(bb);
                            }
                            let fn_name2 = format!("{}_{}", concrete_name, method);
                            if let Some(f) = cg.module.get_function(&fn_name2) {
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
            // Handle super.method() directly
            if let Expr::Super = obj.as_ref() {
                if let Some(ref current) = cg.current_class {
                    if let Some(info) = cg.struct_types.get(current) {
                        if let Some(ref parent) = info.parent {
                            let fn_name = format!("{}_{}", parent, method);
                            if let Some(f) = cg.module.get_function(&fn_name) {
                                let compiled = args.iter().map(|a| cg.compile_expr(a)).collect::<Result<Vec<_>, _>>()?;
                                let mut meta: Vec<BasicMetadataValueEnum> = vec![cg.builder.build_load(cg.context.ptr_type(inkwell::AddressSpace::default()), cg.self_ptr.unwrap(), "self").unwrap().into()];
                                for a in &compiled { meta.push((*a).into()); }
                                let result = cg.builder.build_call(f, &meta, "calltmp").unwrap();
                                return Ok(match result.try_as_basic_value() {
                                    inkwell::values::ValueKind::Basic(bv) => bv,
                                    _ => cg.context.i64_type().const_zero().into(),
                                });
                            }
                        }
                    }
                }
                return Ok(cg.context.i64_type().const_zero().into());
            }

            let obj_val = cg.compile_expr(obj)?;
            let compiled = args.iter().map(|a| cg.compile_expr(a)).collect::<Result<Vec<_>, _>>()?;

            // Walk parent chain to find the most derived class with this method
            let mut best_class: Option<(String, &crate::codegen::ClassInfo)> = None;
            for (class_name, info) in &cg.struct_types {
                if info.methods.iter().any(|m| m == method) {
                    let is_better = match &best_class {
                        Some((best_name, _)) => {
                            // class_name is better if best_name is an ancestor of class_name
                            // (i.e., class_name is more derived than best_name)
                            is_descendant(&cg.struct_types, &best_name, &info.parent)
                                && info.parent.is_some()
                        }
                        None => true,
                    };
                    if is_better {
                        best_class = Some((class_name.clone(), info));
                    }
                }
            }

            if let Some((class_name, _info)) = best_class {
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

            Ok(cg.context.i64_type().const_zero().into())
        }
        _ => unreachable!(),
    }
}

/// Check if `child_name` is a descendant of `parent_name` in the class hierarchy
fn is_descendant(struct_types: &std::collections::HashMap<String, crate::codegen::ClassInfo>, child_name: &str, parent_name: &Option<String>) -> bool {
    match parent_name {
        Some(p) if p == child_name => true,
        Some(p) => {
            if let Some(info) = struct_types.get(p) {
                is_descendant(struct_types, child_name, &info.parent)
            } else {
                false
            }
        }
        None => false,
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
    if cg.module.get_function("strlen").is_none() {
        let ptr_ty = cg.context.ptr_type(inkwell::AddressSpace::default());
        let strlen_ty = cg.context.i64_type().fn_type(&[ptr_ty.into()], false);
        cg.module.add_function("strlen", strlen_ty, None);
    }
    let len = cg.builder.build_call(cg.module.get_function("strlen").unwrap(), &[ptr.into()], "len").unwrap();
    Ok(match len.try_as_basic_value() { inkwell::values::ValueKind::Basic(bv) => bv, _ => cg.context.i64_type().const_zero().into() })
}

fn compile_read<'ctx>(cg: &mut CodeGen<'ctx>) -> Result<BasicValueEnum<'ctx>, AzError> {
    let i64_ty = cg.context.i64_type();
    let i8_ty = cg.context.i8_type();

    // 1024-byte buffer on the stack
    let buf = cg.builder.build_array_alloca(i8_ty, i64_ty.const_int(1024, false), "input_buf").unwrap();

    // Declare getchar
    if cg.module.get_function("getchar").is_none() {
        let getchar_ty = cg.context.i32_type().fn_type(&[], false);
        cg.module.add_function("getchar", getchar_ty, None);
    }

    // Read characters one by one using getchar() in a loop
    // We need a loop: for i = 0..1023 { c = getchar(); if c == '\n' || c == EOF break; buf[i] = c; }
    let cf = cg.function.unwrap();

    // Allocate loop variable i
    let i_ptr = cg.create_entry_alloca(i64_ty.into(), "read_i");

    // Store 0 to i
    cg.builder.build_store(i_ptr, i64_ty.const_zero()).unwrap();

    // Loop header
    let loop_cond = cg.context.append_basic_block(cf, "read_cond");
    let loop_body = cg.context.append_basic_block(cf, "read_body");
    let loop_end = cg.context.append_basic_block(cf, "read_end");
    cg.builder.build_unconditional_branch(loop_cond).unwrap();
    cg.builder.position_at_end(loop_cond);

    let i_val = cg.builder.build_load(i64_ty, i_ptr, "i").unwrap().into_int_value();
    let cmp = cg.builder.build_int_compare(inkwell::IntPredicate::SLT, i_val, i64_ty.const_int(1023, false), "read_cmp").unwrap();
    cg.builder.build_conditional_branch(cmp, loop_body, loop_end).unwrap();
    cg.builder.position_at_end(loop_body);
    cg.loop_stack.push((loop_cond, loop_end));

    // c = getchar()
    let c_raw = cg.builder.build_call(
        cg.module.get_function("getchar").unwrap(),
        &[], "read_char"
    ).unwrap().try_as_basic_value().unwrap_basic().into_int_value();

    // Extend i32 to i64 for comparison
    let c_val = cg.builder.build_int_z_extend(c_raw, i64_ty, "c_ext").unwrap();

    // Check for newline (10) or EOF (-1)
    let is_nl = cg.builder.build_int_compare(inkwell::IntPredicate::EQ, c_val, i64_ty.const_int(10, false), "is_nl").unwrap();
    let is_eof = cg.builder.build_int_compare(inkwell::IntPredicate::EQ, c_val, i64_ty.const_int(0xFFFFFFFF, false), "is_eof").unwrap();
    let should_stop = cg.builder.build_or(is_nl, is_eof, "stop").unwrap();
    let should_stop_bool = cg.builder.build_int_compare(inkwell::IntPredicate::NE, should_stop, cg.context.bool_type().const_zero(), "stop_chk").unwrap();
    let after_store = cg.context.append_basic_block(cf, "read_store");
    cg.builder.build_conditional_branch(should_stop_bool, loop_end, after_store).unwrap();
    cg.builder.position_at_end(after_store);

    // buf[i] = (i8)c
    let c_i8 = cg.builder.build_int_truncate(c_raw, i8_ty, "c_i8").unwrap();
    let i2 = cg.builder.build_load(i64_ty, i_ptr, "i2").unwrap().into_int_value();
    let gep = unsafe { cg.builder.build_gep(i8_ty, buf, &[i2], "chr_gep").unwrap() };
    cg.builder.build_store(gep, c_i8).unwrap();

    // i++
    let i3 = cg.builder.build_load(i64_ty, i_ptr, "i3").unwrap().into_int_value();
    let i_next = cg.builder.build_int_add(i3, i64_ty.const_int(1, false), "i_next").unwrap();
    cg.builder.build_store(i_ptr, i_next).unwrap();

    cg.builder.build_unconditional_branch(loop_cond).unwrap();
    cg.loop_stack.pop();
    cg.builder.position_at_end(loop_end);

    // Null-terminate: buf[i] = 0
    let i_final = cg.builder.build_load(i64_ty, i_ptr, "i_final").unwrap().into_int_value();
    let null_gep = unsafe { cg.builder.build_gep(i8_ty, buf, &[i_final], "null_gep").unwrap() };
    cg.builder.build_store(null_gep, i8_ty.const_zero()).unwrap();

    Ok(buf.into())
}

fn compile_input<'ctx>(cg: &mut CodeGen<'ctx>, args: &[Expr]) -> Result<BasicValueEnum<'ctx>, AzError> {
    // Print the prompt first
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
    // Zero-extend i8 to i64
    Ok(cg.builder.build_int_z_extend(loaded.into_int_value(), cg.context.i64_type(), "ch_ext").unwrap().into())
}

fn compile_chr<'ctx>(cg: &mut CodeGen<'ctx>, args: &[Expr]) -> Result<BasicValueEnum<'ctx>, AzError> {
    let val = cg.compile_expr(&args[0])?;
    let i64_val = val.into_int_value();
    let i8_val = cg.builder.build_int_truncate(i64_val, cg.context.i8_type(), "chr_trunc").unwrap();
    // Use malloc for heap-allocated buffer (stack alloca is freed on return)
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
    let null_gep = unsafe {
        cg.builder.build_gep(cg.context.i8_type(), buf, &[cg.context.i64_type().const_int(1, false)], "null_gep").unwrap()
    };
    cg.builder.build_store(null_gep, cg.context.i8_type().const_zero()).unwrap();
    Ok(buf.into())
}

fn compile_math1<'ctx>(cg: &mut CodeGen<'ctx>, name: &str, args: &[Expr]) -> Result<BasicValueEnum<'ctx>, AzError> {
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

fn compile_math2<'ctx>(cg: &mut CodeGen<'ctx>, name: &str, args: &[Expr]) -> Result<BasicValueEnum<'ctx>, AzError> {
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

fn compile_rand<'ctx>(cg: &mut CodeGen<'ctx>, _args: &[Expr]) -> Result<BasicValueEnum<'ctx>, AzError> {
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
    // Zero-extend i32 to i64
    Ok(cg.builder.build_int_z_extend(val, cg.context.i64_type(), "rand_ext").unwrap().into())
}

fn compile_srand<'ctx>(cg: &mut CodeGen<'ctx>, args: &[Expr]) -> Result<BasicValueEnum<'ctx>, AzError> {
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

fn subst_type_multi(ty: &Type, type_params: &[String], concrete_types: &[String]) -> Type {
    match ty {
        Type::Name(n) => {
            if let Some(idx) = type_params.iter().position(|p| p == n) {
                let mapped = if idx < concrete_types.len() { concrete_types[idx].clone() } else { "int".to_string() };
                Type::Name(mapped)
            } else {
                ty.clone()
            }
        }
        _ => ty.clone(),
    }
}
