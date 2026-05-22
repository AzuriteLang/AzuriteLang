use azurite_parser::ast::Program;

pub struct CodeGen;

impl CodeGen {
    pub fn new() -> Self {
        Self
    }

    pub fn compile_program(&self, _program: &Program) -> Result<(), String> {
        println!("[codegen] LLVM backend not enabled.");
        println!("[codegen] Rebuild with: cargo build --features llvm");
        Ok(())
    }
}
