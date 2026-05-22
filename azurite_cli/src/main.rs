use std::fs;
use std::path::PathBuf;

use clap::Parser as ClapParser;

use azurite_checker::Checker;
use azurite_codegen::CodeGen;
use azurite_lexer::Lexer;
use azurite_parser::Parser;

#[cfg(feature = "llvm")]
use inkwell::context::Context;

#[derive(ClapParser)]
#[command(name = "azurite", about = "AzuriteLang compiler")]
enum Cli {
    #[command(about = "Tokenize source and print tokens")]
    Tokenize {
        #[arg(help = "Path to .az source file")]
        file: PathBuf,
    },
    #[command(about = "Parse source and print AST")]
    Parse {
        #[arg(help = "Path to .az source file")]
        file: PathBuf,
    },
    #[command(about = "Type-check source and report errors")]
    Check {
        #[arg(help = "Path to .az source file")]
        file: PathBuf,
    },
    #[command(about = "Compile source to LLVM IR")]
    Build {
        #[arg(help = "Path to .az source file")]
        file: PathBuf,
        #[arg(short, long, help = "Output file path")]
        output: Option<PathBuf>,
    },
}

fn main() -> Result<(), String> {
    let cli = Cli::parse();

    match cli {
        Cli::Tokenize { file } => {
            let source = read_file(&file)?;
            let mut lexer = Lexer::new(&source);
            let tokens = lexer.tokenize()?;
            for token in &tokens {
                println!("{:?}", token);
            }
        }
        Cli::Parse { file } => {
            let source = read_file(&file)?;
            let (mut parser, _tokens) = Parser::from_source(&source)?;
            let program = parser.parse_program()?;
            println!("{:#?}", program);
        }
        Cli::Check { file } => {
            let source = read_file(&file)?;
            let (mut parser, _tokens) = Parser::from_source(&source)?;
            let program = parser.parse_program()?;
            let mut checker = Checker::new();
            match checker.check_program(&program) {
                Ok(()) => println!("No type errors found."),
                Err(errors) => {
                    for err in &errors {
                        eprintln!("error: {}", err);
                    }
                    return Err(format!("{} type error(s) found", errors.len()));
                }
            }
        }
        Cli::Build { file, output } => {
            let source = read_file(&file)?;
            let (mut parser, _tokens) = Parser::from_source(&source)?;
            let program = parser.parse_program()?;

            #[cfg(feature = "llvm")]
            let codegen = {
                let context = Context::create();
                let mut codegen = CodeGen::new(&context, "azurite_program");
                codegen.compile_program(&program)?;
                let output_path = output.unwrap_or_else(|| file.with_extension("ll"));
                codegen.module().print_to_file(&output_path)
                    .map_err(|e| format!("cannot write output: {}", e))?;
                println!("LLVM IR written to {}", output_path.display());
                codegen
            };

            #[cfg(not(feature = "llvm"))]
            {
                let _ = output;
                let codegen = CodeGen::new();
                codegen.compile_program(&program)?;
            }
        }
    }

    Ok(())
}

fn read_file(path: &PathBuf) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("cannot read {}: {}", path.display(), e))
}
