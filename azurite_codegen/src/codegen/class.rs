use azurite_errors::AzError;
use azurite_parser::ast::*;
use inkwell::types::{BasicMetadataTypeEnum, BasicTypeEnum};
use inkwell::values::BasicValueEnum;
use crate::codegen::{ClassInfo, CodeGen};

pub fn compile_class<'ctx>(cg: &mut CodeGen<'ctx>, name: &Ident, fields: &[ClassField], methods: &[Stmt], parent: &Option<Box<Type>>) -> Result<(), AzError> {
    let has_parent = parent.is_some();
    let mut field_names: Vec<String> = fields.iter().map(|f| f.name.name.clone()).collect();
    let mut field_types: Vec<BasicTypeEnum> = fields.iter().map(|f| cg.field_type_to_llvm(&f.type_)).collect();

    if has_parent {
        field_names.insert(0, "__vtable".to_string());
        field_types.insert(0, cg.context.ptr_type(inkwell::AddressSpace::default()).into());
    }

    let struct_name = format!("struct.{}", name.name);
    let llvm_struct = cg.context.opaque_struct_type(&struct_name);
    llvm_struct.set_body(&field_types, false);

    let method_names: Vec<String> = methods.iter().filter_map(|m| {
        if let Stmt::Func { name: mname, .. } = m { Some(mname.name.clone()) } else { None }
    }).collect();

    let info = ClassInfo {
        field_names: field_names.clone(),
        field_types: field_types.clone(),
        methods: method_names,
        llvm_struct,
        parent: parent.as_ref().and_then(|p| if let Type::Name(n) = p.as_ref() { Some(n.clone()) } else { None }),
        has_vtable: has_parent,
    };
    cg.struct_types.insert(name.name.clone(), info);

    for method in methods {
        if let Stmt::Func { name: mname, params, return_type, body } = method {
            compile_method(cg, name, &llvm_struct, &field_names, mname, params, return_type, body, has_parent)?;
        }
    }
    Ok(())
}

fn compile_method<'ctx>(
    cg: &mut CodeGen<'ctx>,
    class_name: &Ident,
    llvm_struct: &inkwell::types::StructType<'ctx>,
    field_names: &[String],
    mname: &Ident,
    params: &[Param],
    return_type: &Option<Type>,
    body: &Expr,
    _has_parent: bool,
) -> Result<(), AzError> {
    let is_ctor = mname.name == "new";
    let self_type = cg.context.ptr_type(inkwell::AddressSpace::default());

    let fn_name = format!("{}_{}", class_name.name, mname.name);

    if is_ctor {
        let mut param_types: Vec<BasicMetadataTypeEnum> = Vec::new();
        for p in params { param_types.push(cg.az_param_type(&p.type_annotation)); }
        let is_var_args = params.iter().any(|p| p.vararg);
        let ft = cg.context.ptr_type(inkwell::AddressSpace::default()).fn_type(&param_types, is_var_args);
        let fn_val = cg.module.add_function(&fn_name, ft, None);
        let entry = cg.context.append_basic_block(fn_val, "entry");
        cg.builder.position_at_end(entry);
        cg.function = Some(fn_val);
        cg.current_class = Some(class_name.name.clone());

        let instance = cg.builder.build_malloc(*llvm_struct, "instance").unwrap();

        // Store vtable pointer if applicable
        if let Some(info) = cg.struct_types.get(&class_name.name) {
            if info.has_vtable {
                let vtable_name = format!("{}.vtable", class_name.name);
                let vtable_global = cg.module.get_global(&vtable_name);
                if let Some(gv) = vtable_global {
                    let gep = cg.builder.build_struct_gep(*llvm_struct, instance, 0u32, "vptr").unwrap();
                    cg.builder.build_store(gep, gv.as_pointer_value()).unwrap();
                }
            }
        }

        for (i, param) in params.iter().enumerate() {
            if let Some(idx) = field_names.iter().position(|f| f == &param.name.name) {
                if let Some(pv) = fn_val.get_nth_param(i as u32) {
                    // For non-vtable fields, gep + store
                    if idx > 0 || true {
                        let gep = cg.builder.build_struct_gep(*llvm_struct, instance, idx as u32, &param.name.name).unwrap();
                        cg.builder.build_store(gep, pv).unwrap();
                    }
                }
            }
        }

        let loaded: BasicValueEnum = instance.into();
        cg.builder.build_return(Some(&loaded)).unwrap();
        cg.function_defaults.insert(fn_name, params.iter().map(|p| p.default_value.clone()).collect());
        cg.function = None;
        cg.current_class = None;
        return Ok(());
    }

    let is_void = return_type.is_none() || matches!(return_type, Some(Type::Name(ref n)) if n == "void" || n == "none");
    let ret_is_string = matches!(return_type, Some(Type::Name(ref n)) if n == "string");
    let ret_is_float = matches!(return_type, Some(Type::Name(ref n)) if n == "float");
    let ret_name = return_type.as_ref().and_then(|t| if let Type::Name(n) = t { Some(n.as_str()) } else { None });
    let ret_is_instance = !is_void && !ret_is_string && !ret_is_float && ret_name.map_or(false, |n| n != "int" && n != "bool");
    let mut param_types: Vec<BasicMetadataTypeEnum> = vec![self_type.into()];
    for p in params.iter().filter(|p| p.name.name != "self") { param_types.push(cg.az_param_type(&p.type_annotation)); }

    let is_var_args = params.iter().any(|p| p.vararg);
    let fn_val = if is_void {
        let ft = cg.context.void_type().fn_type(&param_types, is_var_args);
        cg.module.add_function(&fn_name, ft, None)
    } else if ret_is_string || ret_is_instance {
        let ft = cg.context.ptr_type(inkwell::AddressSpace::default()).fn_type(&param_types, is_var_args);
        cg.module.add_function(&fn_name, ft, None)
    } else if ret_is_float {
        let ft = cg.context.f64_type().fn_type(&param_types, is_var_args);
        cg.module.add_function(&fn_name, ft, None)
    } else {
        let ft = cg.context.i64_type().fn_type(&param_types, is_var_args);
        cg.module.add_function(&fn_name, ft, None)
    };

    let entry = cg.context.append_basic_block(fn_val, "entry");
    cg.builder.position_at_end(entry);
    cg.function = Some(fn_val);
    cg.current_class = Some(class_name.name.clone());

    if let Some(sp) = fn_val.get_nth_param(0) {
        let sa = cg.create_entry_alloca(self_type.into(), "self_ptr");
        cg.builder.build_store(sa, sp).unwrap();
        cg.self_ptr = Some(sa);
    }

    let mut param_idx = 1u32;
    for param in params.iter().filter(|p| p.name.name != "self") {
        if let Some(pv) = fn_val.get_nth_param(param_idx) {
            let ptr = cg.create_entry_alloca(pv.get_type(), &param.name.name);
            cg.builder.build_store(ptr, pv).unwrap();
            cg.variables.insert(param.name.name.clone(), (ptr, pv.get_type()));
            param_idx += 1;
        }
    }

    let last_val = cg.compile_block_stmts(body, true)?;

    if !cg.has_terminator() {
        if is_void { cg.builder.build_return(None).unwrap(); }
        else if let Some(v) = last_val {
            if ret_is_string || ret_is_float || ret_is_instance {
                cg.builder.build_return(Some(&v)).unwrap();
            } else {
                cg.builder.build_return(Some(&cg.any_to_i64(v))).unwrap();
            }
        }
        else { cg.builder.build_return(Some(&cg.context.i64_type().const_zero())).unwrap(); }
    }

    cg.function_defaults.insert(fn_name, params.iter().map(|p| p.default_value.clone()).collect());
    cg.function = None;
    cg.self_ptr = None;
    cg.current_class = None;
    Ok(())
}
