use azurite_errors::{AzError, ErrorKind};
use azurite_lexer::Span;
use azurite_parser::ast::*;
use inkwell::values::{BasicMetadataValueEnum, BasicValueEnum};
use crate::codegen::CodeGen;
use super::{array, math, io, stri};

pub fn compile_call<'ctx>(cg: &mut CodeGen<'ctx>, expr: &Expr) -> Result<BasicValueEnum<'ctx>, AzError> {
    match expr {
        Expr::Call { callee, args } => {
            let callee_name = match callee.as_ref() {
                Expr::Ident(i) => i.name.clone(),
                _ => return Err(AzError::new(ErrorKind::Semantic, expr.span(), "invalid callee")),
            };
            match callee_name.as_str() {
                "print" => return super::super::builtin::compile_print(cg, args),
                "sqrt" => return math::compile_sqrt(cg, args),
                "abs" => return math::compile_abs(cg, args),
                "sin" | "cos" | "tan" | "log" | "log10" | "floor" | "ceil" | "asin" | "acos" | "atan" | "sinh" | "cosh" | "tanh" | "exp" | "expm1" | "log2" => return math::compile_math1(cg, &callee_name, args),
                "pow" | "atan2" | "hypot" | "fmod" | "copysign" => return math::compile_math2(cg, &callee_name, args),
                "rand" => return math::compile_rand(cg, args),
                "srand" => return math::compile_srand(cg, args),
                "len" | "str" | "int" | "float" | "char_at" | "chr" => return stri::dispatch(cg, &callee_name, args),
                "read" | "input" | "exit" | "getenv" | "system" | "pid" | "cwd" => return io::dispatch(cg, &callee_name, args),
                _ => {}
            }
            let mut compiled = args.iter().map(|a| cg.compile_expr(a)).collect::<Result<Vec<_>, _>>()?;
            if let Some(f) = cg.module.get_function(&callee_name) {
                let total = f.count_params() as usize;
                if compiled.len() < total {
                    let dv = cg.function_defaults.get(&callee_name).cloned().unwrap_or_default();
                    for i in compiled.len()..total.min(dv.len()) {
                        if let Some(ref dv_expr) = dv[i] {
                            compiled.push(cg.compile_expr(dv_expr)?);
                        } else { break; }
                    }
                    while compiled.len() < total {
                        compiled.push(cg.context.i64_type().const_zero().into());
                    }
                }
                let meta: Vec<BasicMetadataValueEnum> = compiled.iter().map(|a| (*a).into()).collect();
                let result = cg.builder.build_call(f, &meta, "calltmp").unwrap();
                Ok(match result.try_as_basic_value() {
                    inkwell::values::ValueKind::Basic(bv) => bv,
                    _ => cg.context.i64_type().const_zero().into(),
                })
            } else {
                Err(AzError::new(ErrorKind::Semantic, expr.span(), format!("undefined '{}'", callee_name)))
            }
        }
        Expr::MethodCall { obj, method, args, null_safe: _ } => {
            // Array method calls: arr.push(), arr.pop(), arr.len()
            if let Expr::Ident(ident) = obj.as_ref() {
                if cg.array_lengths.contains_key(&ident.name) || cg.array_elem_types.contains_key(&ident.name) {
                    let var_name = ident.name.clone();
                    // Load the actual heap pointer from the variable alloca
                    let arr_heap_ptr = if let Some((arr_alloca, _)) = cg.variables.get(&var_name) {
                        cg.builder.build_load(cg.context.ptr_type(inkwell::AddressSpace::default()), *arr_alloca, "arr_ld").unwrap().into_pointer_value()
                    } else {
                        return Err(AzError::new(ErrorKind::Semantic, obj.span(), "unknown array"));
                    };
                    let elem_tag = cg.array_elem_types.get(&var_name).copied().unwrap_or(0);
                    match method.as_str() {
                        "len" => {
                            if let Some(&len_ptr) = cg.array_lengths.get(&var_name) {
                                let val = cg.builder.build_load(cg.context.i64_type(), len_ptr, "arr_len").unwrap();
                                return Ok(val);
                            }
                            // Fallback: read from heap header
                            let hdr = unsafe { cg.builder.build_gep(cg.context.i64_type(), arr_heap_ptr, &[cg.context.i64_type().const_int(-1i64 as u64, true)], "hdr").unwrap() };
                            let val = cg.builder.build_load(cg.context.i64_type(), hdr, "arr_len").unwrap();
                            return Ok(val);
                        }
                        "is_empty" => {
                            let len = super::control::read_array_len(cg, arr_heap_ptr);
                            let zero = cg.context.i64_type().const_zero();
                            let is_zero = cg.builder.build_int_compare(inkwell::IntPredicate::EQ, len, zero, "emp").unwrap();
                            return Ok(cg.builder.build_int_z_extend(is_zero, cg.context.i64_type(), "emp_i64").unwrap().into());
                        }
                        "clear" => {
                            super::control::write_array_len(cg, arr_heap_ptr, cg.context.i64_type().const_zero());
                            if let Some(&len_ptr) = cg.array_lengths.get(&var_name) {
                                cg.builder.build_store(len_ptr, cg.context.i64_type().const_zero()).unwrap();
                            }
                            return Ok(cg.context.i64_type().const_zero().into());
                        }
                        "reverse" => {
                            array::compile_array_reverse(cg, &var_name, arr_heap_ptr, elem_tag);
                            return Ok(cg.context.i64_type().const_zero().into());
                        }
                        "sort" => {
                            array::compile_array_sort(cg, &var_name, arr_heap_ptr, elem_tag);
                            return Ok(cg.context.i64_type().const_zero().into());
                        }
                        "contains" => {
                            let val = cg.compile_expr(&args[0])?;
                            let val_i64 = super::control::val_to_i64(cg, val);
                            let result = array::compile_array_contains(cg, arr_heap_ptr, &var_name, val_i64, elem_tag);
                            return Ok(result);
                        }
                        "push" => {
                            let val = cg.compile_expr(&args[0])?;
                            let val_i64 = super::control::val_to_i64(cg, val);
                            let i64_ty = cg.context.i64_type();
                            let old_len = super::control::read_array_len(cg, arr_heap_ptr);
                            let new_len = cg.builder.build_int_add(old_len, i64_ty.const_int(1, false), "nl").unwrap();
                            super::control::write_array_len(cg, arr_heap_ptr, new_len);
                            if let Some(&len_ptr) = cg.array_lengths.get(&var_name) {
                                cg.builder.build_store(len_ptr, new_len).unwrap();
                            }
                            // Realloc if capacity entry exists
                            if let Some(&cap_ptr) = cg.array_lengths.get(&format!("{}.__cap", var_name)) {
                                let old_cap = cg.builder.build_load(i64_ty, cap_ptr, "oc").unwrap().into_int_value();
                                let full = cg.builder.build_int_compare(inkwell::IntPredicate::EQ, old_len, old_cap, "full").unwrap();
                                let cf = cg.function.unwrap();
                                let grow_bb = cg.context.append_basic_block(cf, "grow");
                                let skip_bb = cg.context.append_basic_block(cf, "nogrow");
                                let merge_bb = cg.context.append_basic_block(cf, "push_end");
                                cg.builder.build_conditional_branch(full, grow_bb, skip_bb).unwrap();
                                cg.builder.position_at_end(grow_bb);
                                let zero_cap = cg.builder.build_int_compare(inkwell::IntPredicate::EQ, old_cap, i64_ty.const_zero(), "zc").unwrap();
                                let base_cap = i64_ty.const_int(4, false);
                                let doubled = cg.builder.build_int_mul(old_cap, i64_ty.const_int(2, false), "dbl").unwrap();
                                let new_cap = cg.builder.build_select(zero_cap, base_cap, doubled, "nc").unwrap().into_int_value();
                                let new_sz = cg.builder.build_int_mul(new_cap, i64_ty.const_int(8, false), "nsz").unwrap();
                                if cg.module.get_function("realloc").is_none() {
                                    let ptr_ty = cg.context.ptr_type(inkwell::AddressSpace::default());
                                    let rt = ptr_ty.fn_type(&[ptr_ty.into(), i64_ty.into()], false);
                                    cg.module.add_function("realloc", rt, None);
                                }
                                // Compute raw pointer (header starts at data_ptr - 1)
                                let raw_int = cg.builder.build_int_sub(cg.builder.build_ptr_to_int(arr_heap_ptr, i64_ty, "dp2i").unwrap(), i64_ty.const_int(8, false), "raw_i").unwrap();
                                let raw_ptr = cg.builder.build_int_to_ptr(raw_int, cg.context.ptr_type(inkwell::AddressSpace::default()), "raw_p").unwrap();
                                let realloc_sz = cg.builder.build_int_add(new_sz, i64_ty.const_int(8, false), "rsz").unwrap();
                                let new_raw_call = cg.builder.build_call(cg.module.get_function("realloc").unwrap(), &[raw_ptr.into(), realloc_sz.into()], "rp").unwrap();
                                let new_raw_pv = new_raw_call.try_as_basic_value().unwrap_basic().into_pointer_value();
                                // New data pointer is new_raw + 1
                                let new_data = unsafe { cg.builder.build_gep(i64_ty, new_raw_pv, &[i64_ty.const_int(1, false)], "nd").unwrap() };
                                cg.builder.build_store(cap_ptr, new_cap).unwrap();
                                if let Some(&(arr_alloca, _)) = cg.variables.get(&var_name) {
                                    cg.builder.build_store(arr_alloca, new_data).unwrap();
                                }
                                cg.builder.build_unconditional_branch(merge_bb).unwrap();
                                cg.builder.position_at_end(skip_bb);
                                cg.builder.build_unconditional_branch(merge_bb).unwrap();
                                cg.builder.position_at_end(merge_bb);
                            }
                            let arr_final = if let Some((arr_alloca, _)) = cg.variables.get(&var_name) {
                                cg.builder.build_load(cg.context.ptr_type(inkwell::AddressSpace::default()), *arr_alloca, "arr_f").unwrap().into_pointer_value()
                            } else { arr_heap_ptr };
                            let gep = unsafe { cg.builder.build_gep(i64_ty, arr_final, &[old_len], "push_gep").unwrap() };
                            cg.builder.build_store(gep, val_i64).unwrap();
                            return Ok(i64_ty.const_zero().into());
                        }
                        "pop" => {
                            let i64_ty = cg.context.i64_type();
                            let old_len = super::control::read_array_len(cg, arr_heap_ptr);
                            let new_len = cg.builder.build_int_sub(old_len, i64_ty.const_int(1, false), "nl").unwrap();
                            let is_empty = cg.builder.build_int_compare(inkwell::IntPredicate::EQ, old_len, i64_ty.const_zero(), "emp").unwrap();
                            let cf = cg.function.unwrap();
                            let not_empty_bb = cg.context.append_basic_block(cf, "pne");
                            let empty_bb = cg.context.append_basic_block(cf, "pemp");
                            let mg = cg.context.append_basic_block(cf, "pmg");
                            let res_alloca = cg.create_entry_alloca(i64_ty.into(), "pop_res");
                            cg.builder.build_store(res_alloca, i64_ty.const_zero()).unwrap();
                            cg.builder.build_conditional_branch(is_empty, empty_bb, not_empty_bb).unwrap();
                            cg.builder.position_at_end(not_empty_bb);
                            super::control::write_array_len(cg, arr_heap_ptr, new_len);
                            if let Some(&len_ptr) = cg.array_lengths.get(&var_name) {
                                cg.builder.build_store(len_ptr, new_len).unwrap();
                            }
                            let last_idx = cg.builder.build_int_sub(old_len, i64_ty.const_int(1, false), "li").unwrap();
                            let gep = unsafe { cg.builder.build_gep(i64_ty, arr_heap_ptr, &[last_idx], "pop_gep").unwrap() };
                            let raw = cg.builder.build_load(i64_ty, gep, "pop_v").unwrap();
                            cg.builder.build_store(res_alloca, raw).unwrap();
                            cg.builder.build_unconditional_branch(mg).unwrap();
                            cg.builder.position_at_end(empty_bb);
                            cg.builder.build_unconditional_branch(mg).unwrap();
                            cg.builder.position_at_end(mg);
                            let result = cg.builder.build_load(i64_ty, res_alloca, "pop_r").unwrap();
                            return Ok(result);
                        }
                        "insert" => {
                            let i64_ty = cg.context.i64_type();
                            array::compile_array_insert(cg, &var_name, arr_heap_ptr, elem_tag, &args[0], &args[1])?;
                            return Ok(i64_ty.const_zero().into());
                        }
                        "remove" => {
                            let result = array::compile_array_remove(cg, &var_name, arr_heap_ptr, elem_tag, &args[0])?;
                            return Ok(result);
                        }
                        "map" => {
                            let fn_name = if let Expr::Ident(i) = &args[0] { i.name.clone() } else { return Err(AzError::new(ErrorKind::Semantic, args[0].span(), "expected function name")); };
                            let result = array::compile_array_map(cg, &var_name, arr_heap_ptr, elem_tag, &fn_name)?;
                            return Ok(result);
                        }
                        "filter" => {
                            let fn_name = if let Expr::Ident(i) = &args[0] { i.name.clone() } else { return Err(AzError::new(ErrorKind::Semantic, args[0].span(), "expected function name")); };
                            let result = array::compile_array_filter(cg, &var_name, arr_heap_ptr, elem_tag, &fn_name)?;
                            return Ok(result);
                        }
                        "reduce" => {
                            let fn_name = if let Expr::Ident(i) = &args[1] { i.name.clone() } else { return Err(AzError::new(ErrorKind::Semantic, args[1].span(), "expected function name")); };
                            let result = array::compile_array_reduce(cg, &var_name, arr_heap_ptr, elem_tag, &args[0], &fn_name)?;
                            return Ok(result);
                        }
                        _ => {}
                    }
                }
            }
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
                                    vararg: false,
                                    default_value: None,
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
                                let mut compiled = args.iter().map(|a| cg.compile_expr(a)).collect::<Result<Vec<_>, _>>()?;
                                let total = f.count_params() as usize;
                                if compiled.len() < total {
                                    let dv = cg.function_defaults.get(&fn_name2).cloned().unwrap_or_default();
                                    for i in compiled.len()..total.min(dv.len()) {
                                        if let Some(ref dv_expr) = dv[i] {
                                            compiled.push(cg.compile_expr(dv_expr)?);
                                        } else {
                                            compiled.push(cg.context.i64_type().const_zero().into());
                                        }
                                    }
                                    while compiled.len() < total {
                                        compiled.push(cg.context.i64_type().const_zero().into());
                                    }
                                }
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
                        let mut compiled = args.iter().map(|a| cg.compile_expr(a)).collect::<Result<Vec<_>, _>>()?;
                        let total = f.count_params() as usize;
                        if compiled.len() < total {
                            let dv = cg.function_defaults.get(&fn_name).cloned().unwrap_or_default();
                            for i in compiled.len()..total.min(dv.len()) {
                                if let Some(ref dv_expr) = dv[i] {
                                    compiled.push(cg.compile_expr(dv_expr)?);
                                } else {
                                    compiled.push(cg.context.i64_type().const_zero().into());
                                }
                            }
                            while compiled.len() < total {
                                compiled.push(cg.context.i64_type().const_zero().into());
                            }
                        }
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
                                let mut compiled = args.iter().map(|a| cg.compile_expr(a)).collect::<Result<Vec<_>, _>>()?;
                                let total = f.count_params() as usize;
                                if 1 + compiled.len() < total {
                                    let dv = cg.function_defaults.get(&fn_name).cloned().unwrap_or_default();
                                    for i in (1 + compiled.len())..total.min(dv.len()) {
                                        if let Some(ref dv_expr) = dv[i] {
                                            compiled.push(cg.compile_expr(dv_expr)?);
                                        } else {
                                            compiled.push(cg.context.i64_type().const_zero().into());
                                        }
                                    }
                                    while 1 + compiled.len() < total {
                                        compiled.push(cg.context.i64_type().const_zero().into());
                                    }
                                }
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
            let mut compiled = args.iter().map(|a| cg.compile_expr(a)).collect::<Result<Vec<_>, _>>()?;

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
                    let total = f.count_params() as usize;
                    if 1 + compiled.len() < total {
                        let dv = cg.function_defaults.get(&fn_name).cloned().unwrap_or_default();
                        for i in (1 + compiled.len())..total.min(dv.len()) {
                            if let Some(ref dv_expr) = dv[i] {
                                compiled.push(cg.compile_expr(dv_expr)?);
                            } else {
                                compiled.push(cg.context.i64_type().const_zero().into());
                            }
                        }
                        while 1 + compiled.len() < total {
                            compiled.push(cg.context.i64_type().const_zero().into());
                        }
                    }
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
