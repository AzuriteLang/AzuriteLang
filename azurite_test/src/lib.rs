use azurite_lexer::Lexer;
use azurite_parser::ast::Program;
use azurite_parser::Parser;

pub fn parse_prog(src: &str) -> Program {
    let tokens = Lexer::new(src).tokenize().unwrap();
    Parser::new(tokens).parse_program().unwrap()
}

pub fn check(src: &str) -> Result<(), Vec<azurite_errors::AzError>> {
    let tokens = azurite_lexer::Lexer::new(src).tokenize().unwrap();
    let prog = azurite_parser::Parser::new(tokens).parse_program().unwrap();
    azurite_checker::Checker::new().check_program(&prog)
}

#[cfg(feature = "llvm")]
pub fn run(src: &str) -> Result<(), String> {
    use inkwell::execution_engine::JitFunction;
    use azurite_codegen::CodeGen;
    let context = inkwell::context::Context::create();
    let tokens = azurite_lexer::Lexer::new(src).tokenize().map_err(|e| e.to_string())?;
    let prog = azurite_parser::Parser::new(tokens).parse_program().map_err(|e| e.to_string())?;
    let mut checker = azurite_checker::Checker::new();
    checker.check_program(&prog).map_err(|errs| errs.iter().map(|e| e.message.clone()).collect::<Vec<_>>().join("\n"))?;
    let mut cg = CodeGen::new(&context, "test_module");
    cg.compile_program(&prog).map_err(|e| e.to_string())?;
    let ee = cg.module().create_jit_execution_engine(inkwell::OptimizationLevel::None).map_err(|e| format!("JIT error: {}", e))?;
    unsafe {
        let func: JitFunction<unsafe extern "C" fn()> = ee.get_function("main").map_err(|e| format!("JIT get_function: {}", e))?;
        func.call();
    }
    Ok(())
}
