use std::fs;
use std::path::PathBuf;

use clap::Parser as ClapParser;

use azurite_checker::Checker;
use azurite_codegen::CodeGen;
use azurite_errors::Diagnostic;
use azurite_lexer::Lexer;
use azurite_parser::Parser;

#[cfg(feature = "llvm")]
use inkwell::context::Context;

#[derive(ClapParser)]
#[command(name = "azurite", about = "AzuriteLang compiler")]
enum Cli {
    #[command(about = "Tokenize source and print tokens")]
    Tokenize { file: PathBuf },
    #[command(about = "Parse source and print AST")]
    Parse { file: PathBuf },
    #[command(about = "Type-check source and report errors")]
    Check { file: PathBuf },
    #[command(about = "Compile source to LLVM IR")]
    Build {
        file: PathBuf,
        #[arg(short, long, help = "Output file path")]
        output: Option<PathBuf>,
    },
}

fn main() {
    let cli = Cli::parse();

    let result = match &cli {
        Cli::Tokenize { file } => cmd_tokenize(file),
        Cli::Parse { file } => cmd_parse(file),
        Cli::Check { file } => cmd_check(file),
        Cli::Build { file, output } => cmd_build(file, output.as_ref()),
    };

    if let Err(_msg) = result {
        std::process::exit(1);
    }
}

fn read_file(path: &PathBuf) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("cannot read {}: {}", path.display(), e))
}

fn cmd_tokenize(file: &PathBuf) -> Result<(), String> {
    let source = read_file(file)?;
    match Lexer::new(&source).tokenize() {
        Ok(tokens) => {
            for token in &tokens {
                println!("{:?}", token);
            }
            Ok(())
        }
        Err(msg) => {
            let err = azurite_errors::AzError::new(
                azurite_errors::ErrorKind::Lex,
                azurite_lexer::Span::new(0, 0, 1, 1),
                msg,
            );
            Diagnostic::print(&source, &file.to_string_lossy(), &err);
            Err("tokenization failed".to_string())
        }
    }
}

fn cmd_parse(file: &PathBuf) -> Result<(), String> {
    let source = read_file(file)?;
    match Parser::from_source(&source) {
        Ok((mut parser, _tokens)) => match parser.parse_program() {
            Ok(program) => {
                println!("{:#?}", program);
                Ok(())
            }
            Err(err) => {
                Diagnostic::print(&source, &file.to_string_lossy(), &err);
                Err("parse failed".to_string())
            }
        },
        Err(err) => {
            Diagnostic::print(&source, &file.to_string_lossy(), &err);
            Err("parse failed".to_string())
        }
    }
}

fn cmd_check(file: &PathBuf) -> Result<(), String> {
    let source = read_file(file)?;
    match Parser::from_source(&source) {
        Ok((mut parser, _tokens)) => match parser.parse_program() {
            Ok(program) => {
                let mut checker = Checker::new();
                match checker.check_program(&program) {
                    Ok(()) => {
                        println!("No type errors found.");
                        Ok(())
                    }
                    Err(errors) => {
                        for err in &errors {
                            Diagnostic::print(&source, &file.to_string_lossy(), err);
                        }
                        Err(format!("{} type error(s) found", errors.len()))
                    }
                }
            }
            Err(err) => {
                Diagnostic::print(&source, &file.to_string_lossy(), &err);
                Err("parse failed".to_string())
            }
        },
        Err(err) => {
            Diagnostic::print(&source, &file.to_string_lossy(), &err);
            Err("parse failed".to_string())
        }
    }
}

fn cmd_build(file: &PathBuf, _output: Option<&PathBuf>) -> Result<(), String> {
    let source = read_file(file)?;
    match Parser::from_source(&source) {
        Ok((mut parser, _tokens)) => match parser.parse_program() {
            Ok(_program) => {
                #[cfg(feature = "llvm")]
                {
                    let context = Context::create();
                    let mut codegen = CodeGen::new(&context, "azurite_program");
                    codegen.compile_program(&_program).map_err(|e| e.to_string())?;
                    let output_path = _output.unwrap_or(&file.with_extension("ll")).clone();
                    codegen.module().print_to_file(&output_path)
                        .map_err(|e| format!("cannot write output: {}", e))?;
                    println!("LLVM IR written to {}", output_path.display());
                }
                #[cfg(not(feature = "llvm"))]
                {
                    let _ = _output;
                    let codegen = CodeGen::new();
                    codegen.compile_program(&_program).map_err(|e| e.to_string())?;
                }
                Ok(())
            }
            Err(err) => {
                Diagnostic::print(&source, &file.to_string_lossy(), &err);
                Err("build failed".to_string())
            }
        },
        Err(err) => {
            Diagnostic::print(&source, &file.to_string_lossy(), &err);
            Err("build failed".to_string())
        }
    }
}
