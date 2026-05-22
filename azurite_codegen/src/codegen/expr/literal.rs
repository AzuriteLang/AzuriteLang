use azurite_errors::AzError;
use azurite_parser::ast::*;
use inkwell::values::BasicValueEnum;
use crate::codegen::CodeGen;

pub fn compile_literal<'ctx>(cg: &mut CodeGen<'ctx>, expr: &Expr) -> Result<BasicValueEnum<'ctx>, AzError> {
    match expr {
        Expr::Int(n) => Ok(cg.context.i64_type().const_int(*n as u64, false).into()),
        Expr::Float(n) => Ok(cg.context.f64_type().const_float(*n).into()),
        Expr::String(s) => {
            let ptr = cg.builder.build_global_string_ptr(s, "str").unwrap();
            Ok(ptr.as_pointer_value().into())
        }
        Expr::Bool(b) => Ok(cg.context.i64_type().const_int(*b as u64, false).into()),
        Expr::Null => Ok(cg.context.i64_type().const_zero().into()),
        Expr::Char(c) => Ok(cg.context.i64_type().const_int(*c as u64, false).into()),
        Expr::Self_ | Expr::Super => {
            match cg.self_ptr {
                Some(ptr) => {
                    let loaded = cg.builder.build_load(
                        cg.context.ptr_type(inkwell::AddressSpace::default()), ptr, "self",
                    ).unwrap();
                    Ok(loaded)
                }
                None => Err(AzError::new(azurite_errors::ErrorKind::Semantic, azurite_lexer::Span::new(0, 0, 0, 0),
                    "'self' used outside method"))
            }
        }
        _ => unreachable!(),
    }
}
