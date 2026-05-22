use azurite_errors::AzError;
use azurite_parser::ast::*;
use inkwell::types::{BasicMetadataTypeEnum, BasicTypeEnum};
use inkwell::values::{BasicMetadataValueEnum, BasicValueEnum};
use crate::codegen::{ClassInfo, CodeGen};

pub fn compile_class<'ctx>(cg: &mut CodeGen<'ctx>, name: &Ident, fields: &[ClassField], methods: &[Stmt]) -> Result<(), AzError> {
    let field_names: Vec<String> = fields.iter().map(|f| f.name.name.clone()).collect();
    let field_types: Vec<BasicTypeEnum> = fields.iter().map(|f| cg.field_type_to_llvm(&f.type_)).collect();

    let struct_name = format!("struct.{}", name.name);
    let llvm_struct = cg.context.opaque_struct_type(&struct_name);
    llvm_struct.set_body(&field_types, false);

    let mut method_names = Vec::new();
    for m in methods {
        if let Stmt::Func { name: mname, .. } = m {
            method_names.push(mname.name.clone());
        }
    }

    let info = ClassInfo { field_names: field_names.clone(), field_types: field_types.clone(), methods: method_names, llvm_struct };
    cg.struct_types.insert(name.name.clone(), info);

    for method in methods {
        if let Stmt::Func { name: mname, params, return_type, body } = method {
            compile_method(cg, name, &llvm_struct, &field_names, &field_types, mname, params, return_type, body)?;
        }
    }

    Ok(())
}

fn compile_method<'ctx>(
    cg: &mut CodeGen<'ctx>,
    class_name: &Ident,
    llvm_struct: &inkwell::types::StructType<'ctx>,
    field_names: &[String],
    field_types: &[BasicTypeEnum<'ctx>],
    mname: &Ident,
    params: &[Param],
    return_type: &Option<Type>,
    body: &Expr,
) -> Result<(), AzError> {
    let is_ctor = mname.name == "new";
    let self_type = cg.context.ptr_type(inkwell::AddressSpace::default());
    let struct_ptr_type: BasicTypeEnum = cg.context.ptr_type(inkwell::AddressSpace::default()).into();

    let fn_name = format!("{}_{}", class_name.name, mname.name);

    if is_ctor {
        // Constructor: returns ptr to struct
        let mut param_types: Vec<BasicMetadataTypeEnum> = Vec::new();
        for p in params {
            param_types.push(cg.az_param_type(&p.type_annotation));
        }

        let ft = cg.context.ptr_type(inkwell::AddressSpace::default()).fn_type(&param_types, false);
        let fn_val = cg.module.add_function(&fn_name, ft, None);
        let entry = cg.context.append_basic_block(fn_val, "entry");
        cg.builder.position_at_end(entry);
        cg.function = Some(fn_val);
        cg.current_class = Some(class_name.name.clone());

        // Allocate struct on the heap
        let instance = cg.builder.build_malloc(*llvm_struct, "instance").unwrap();

        // Store each param into the corresponding field
        for (i, param) in params.iter().enumerate() {
            if let Some(idx) = field_names.iter().position(|f| f == &param.name.name) {
                if let Some(pv) = fn_val.get_nth_param(i as u32) {
                    let gep = cg.builder.build_struct_gep(*llvm_struct, instance, idx as u32, &param.name.name).unwrap();
                    cg.builder.build_store(gep, pv).unwrap();
                }
            }
        }

        // Return the struct pointer
        cg.builder.build_return(Some(&instance)).unwrap();

        cg.function = None;
        cg.current_class = None;
        return Ok(());
    }

    // Regular method: first param is self
    let is_void = return_type.is_none() || matches!(return_type, Some(Type::Name(ref n)) if n == "void" || n == "none");

    let mut param_types: Vec<BasicMetadataTypeEnum> = vec![self_type.into()];
    for p in params {
        param_types.push(cg.az_param_type(&p.type_annotation));
    }

    let fn_val = if is_void {
        let ft = cg.context.void_type().fn_type(&param_types, false);
        cg.module.add_function(&fn_name, ft, None)
    } else {
        let ft = cg.context.i64_type().fn_type(&param_types, false);
        cg.module.add_function(&fn_name, ft, None)
    };

    let entry = cg.context.append_basic_block(fn_val, "entry");
    cg.builder.position_at_end(entry);
    cg.function = Some(fn_val);
    cg.current_class = Some(class_name.name.clone());

    // Self param
    if let Some(sp) = fn_val.get_nth_param(0) {
        let sa = cg.create_entry_alloca(self_type.into(), "self_ptr");
        cg.builder.build_store(sa, sp).unwrap();
        cg.self_ptr = Some(sa);
    }

    // Other params (skip self - it's passed as the first implicit arg)
    let mut param_idx = 1u32;
    for param in params.iter().filter(|p| p.name.name != "self") {
        if let Some(pv) = fn_val.get_nth_param(param_idx) {
            let ptr = cg.create_entry_alloca(pv.get_type(), &param.name.name);
            cg.builder.build_store(ptr, pv).unwrap();
            cg.variables.insert(param.name.name.clone(), (ptr, pv.get_type()));
            param_idx += 1;
        }
    }

    cg.compile_block_stmts(body, true)?;

    if !cg.has_terminator() {
        if is_void { cg.builder.build_return(None).unwrap(); }
        else { cg.builder.build_return(Some(&cg.context.i64_type().const_zero())).unwrap(); }
    }

    cg.function = None;
    cg.self_ptr = None;
    cg.current_class = None;
    Ok(())
}

pub fn compile_field_access<'ctx>(cg: &mut CodeGen<'ctx>, obj: &Expr, field: &str) -> Result<BasicValueEnum<'ctx>, AzError> {
    let obj_val = cg.compile_expr(obj)?;
    let ptr = obj_val.into_pointer_value();

    // If accessing from self, we know the class
    if let Some(ref class_name) = cg.current_class {
        if let Some(info) = cg.struct_types.get(class_name) {
            if let Some(idx) = info.field_names.iter().position(|f| f == field) {
                if let Some(ft) = info.field_types.get(idx) {
                    let gep = cg.builder.build_struct_gep(info.llvm_struct, ptr, idx as u32, field).unwrap();
                    let loaded = cg.builder.build_load(*ft, gep, field).unwrap();
                    return Ok(loaded);
                }
            }
        }
    }

    // Fallback: search all classes
    for (_, info) in &cg.struct_types {
        if let Some(idx) = info.field_names.iter().position(|f| f == field) {
            if let Some(ft) = info.field_types.get(idx) {
                let gep = cg.builder.build_struct_gep(info.llvm_struct, ptr, idx as u32, field).unwrap();
                let loaded = cg.builder.build_load(*ft, gep, field).unwrap();
                return Ok(loaded);
            }
        }
    }

    Ok(cg.context.i64_type().const_zero().into())
}

pub fn compile_method_call<'ctx>(cg: &mut CodeGen<'ctx>, obj: &Expr, method: &str, args: &[Expr]) -> Result<BasicValueEnum<'ctx>, AzError> {
    // Check for constructor call: ClassName.new(...)
    if method == "new" {
        if let Expr::Ident(ident) = obj {
            let fn_name = format!("{}_{}", ident.name, method);
            if let Some(f) = cg.module.get_function(&fn_name) {
                let compiled = args.iter()
                    .map(|a| cg.compile_expr(a))
                    .collect::<Result<Vec<_>, _>>()?;
                let meta: Vec<BasicMetadataValueEnum> = compiled.iter().map(|a| (*a).into()).collect();
                let result = cg.builder.build_call(f, &meta, "calltmp").unwrap();
                return Ok(match result.try_as_basic_value() {
                    inkwell::values::ValueKind::Basic(bv) => bv,
                    _ => cg.context.i64_type().const_zero().into(),
                });
            }
        }
    }

    // Regular method call: instance.method(args)
    let obj_val = cg.compile_expr(obj)?;

    let compiled_args = args.iter()
        .map(|a| cg.compile_expr(a))
        .collect::<Result<Vec<_>, _>>()?;

    for (class_name, info) in &cg.struct_types {
        if info.methods.iter().any(|m| m == method) {
            let fn_name = format!("{}_{}", class_name, method);
            if let Some(f) = cg.module.get_function(&fn_name) {
                let mut meta: Vec<BasicMetadataValueEnum> = vec![obj_val.into()];
                for a in &compiled_args { meta.push((*a).into()); }
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
