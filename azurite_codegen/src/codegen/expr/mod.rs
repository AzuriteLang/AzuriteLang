use azurite_errors::{AzError, ErrorKind};
use azurite_parser::ast::*;
use inkwell::values::BasicValueEnum;
use crate::codegen::CodeGen;

mod literal;
mod operator;
mod call;
pub(crate) mod control;
pub(crate) mod array;
pub(crate) mod math;
pub(crate) mod io;
pub(crate) mod stri;

pub fn compile_expr<'ctx>(cg: &mut CodeGen<'ctx>, expr: &Expr) -> Result<BasicValueEnum<'ctx>, AzError> {
    match expr {
        Expr::Int(_) | Expr::Float(_) | Expr::String(_) | Expr::Bool(_) | Expr::Null | Expr::Char(_) | Expr::Self_ | Expr::Super
            => literal::compile_literal(cg, expr),
        Expr::Binary { .. } | Expr::Unary { .. } => operator::compile_operator(cg, expr),
        Expr::Call { .. } | Expr::MethodCall { .. } => call::compile_call(cg, expr),
        Expr::If { .. } | Expr::While { .. } | Expr::Match { .. } | Expr::Block(_) | Expr::Array(_) | Expr::Index { .. } | Expr::Slice { .. } | Expr::Range { .. } | Expr::EnumVariant { .. } | Expr::FieldAccess { .. } | Expr::Tuple(_)
            => control::compile_control(cg, expr),
        Expr::Ident(ident) => {
            if ident.name == "self" {
                if let Some(sp) = cg.self_ptr {
                    return Ok(cg.builder.build_load(cg.context.ptr_type(inkwell::AddressSpace::default()), sp, "self").unwrap());
                }
            }
            if let Some((ptr, ty)) = cg.variables.get(&ident.name) {
                Ok(cg.builder.build_load(*ty, *ptr, &ident.name).unwrap())
            } else if let Some(f) = cg.module.get_function(&ident.name) {
                let result = cg.builder.build_call(f, &[], "calltmp").unwrap();
                Ok(match result.try_as_basic_value() {
                    inkwell::values::ValueKind::Basic(bv) => bv,
                    _ => cg.context.i64_type().const_zero().into(),
                })
            } else if is_class_name(cg, &ident.name) || is_enum_name(cg, &ident.name) {
                Ok(cg.context.i64_type().const_zero().into())
            } else {
                Err(AzError::new(ErrorKind::Semantic, ident.span, format!("undefined '{}'", ident.name)))
            }
        }
    }
}

fn is_class_name<'ctx>(cg: &CodeGen<'ctx>, name: &str) -> bool {
    cg.struct_types.contains_key(name)
}

fn is_enum_name<'ctx>(cg: &CodeGen<'ctx>, name: &str) -> bool {
    cg.enums.contains_key(name)
}
