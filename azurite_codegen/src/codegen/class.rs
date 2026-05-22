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

    let info = ClassInfo {
        field_names: field_names.clone(),
        field_types: field_types.clone(),
        methods: method_names,
        llvm_struct,
    };
    cg.struct_types.insert(name.name.clone(), info);

    for method in methods {
        if let Stmt::Func { name: mname, params, return_type, body } = method {
            compile_method(cg, name, mname, params, return_type, body)?;
        }
    }

    Ok(())
}

fn compile_method<'ctx>(cg: &mut CodeGen<'ctx>, class_name: &Ident, mname: &Ident, params: &[Param], return_type: &Option<Type>, body: &Expr) -> Result<(), AzError> {
    let is_void = return_type.is_none() || matches!(return_type, Some(Type::Name(ref n)) if n == "void" || n == "none");
    let self_type = cg.context.ptr_type(inkwell::AddressSpace::default());

    let mut param_types: Vec<BasicMetadataTypeEnum> = vec![self_type.into()];
    for p in params {
        param_types.push(cg.az_param_type(&p.type_annotation));
    }

    let fn_name = format!("{}_{}", class_name.name, mname.name);
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

    if let Some(sp) = fn_val.get_nth_param(0) {
        let sa = cg.create_entry_alloca(
            BasicTypeEnum::PointerType(cg.context.ptr_type(inkwell::AddressSpace::default())),
            "self_ptr",
        );
        cg.builder.build_store(sa, sp).unwrap();
        cg.self_ptr = Some(sa);
    }

    for (i, param) in params.iter().enumerate() {
        if let Some(pv) = fn_val.get_nth_param((i + 1) as u32) {
            let ptr = cg.create_entry_alloca(pv.get_type(), &param.name.name);
            cg.builder.build_store(ptr, pv).unwrap();
            cg.variables.insert(param.name.name.clone(), (ptr, pv.get_type()));
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

    for (_, info) in &cg.struct_types {
        if let Some(idx) = info.field_names.iter().position(|f| f == field) {
            if let Some(field_type) = info.field_types.get(idx) {
                let gep = cg.builder.build_struct_gep(info.llvm_struct, ptr, idx as u32, field).unwrap();
                let loaded = cg.builder.build_load(*field_type, gep, field).unwrap();
                return Ok(loaded);
            }
        }
    }

    Ok(cg.context.i64_type().const_zero().into())
}

pub fn compile_method_call<'ctx>(cg: &mut CodeGen<'ctx>, obj: &Expr, method: &str, args: &[Expr]) -> Result<BasicValueEnum<'ctx>, AzError> {
    let obj_val = cg.compile_expr(obj)?;

    let compiled_args = args.iter()
        .map(|a| cg.compile_expr(a))
        .collect::<Result<Vec<_>, _>>()?;

    // Try to find the method in any known class
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
