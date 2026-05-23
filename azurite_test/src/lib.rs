use azurite_lexer::Lexer;
use azurite_parser::ast::Program;
use azurite_parser::Parser;

pub fn parse_prog(src: &str) -> Program {
    let tokens = Lexer::new(src).tokenize().unwrap();
    Parser::new(tokens).parse_program().unwrap()
}

pub fn check(src: &str) -> Result<(), Vec<azurite_errors::AzError>> {
    let tokens = Lexer::new(src).tokenize().unwrap();
    let prog = Parser::new(tokens).parse_program().unwrap();
    azurite_checker::Checker::new().check_program(&prog)
}
