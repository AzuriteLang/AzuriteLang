use std::fs;
use std::path::{Path, PathBuf};

use clap::Parser as ClapParser;

use azurite_checker::Checker;
#[cfg(feature = "llvm")]
use azurite_codegen::CodeGen;
use azurite_errors::Diagnostic;
use azurite_lexer::Lexer;
use azurite_parser::ast::{Program, Stmt};
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
    #[command(about = "Interactive REPL")]
    Repl,
}

fn main() {
    let cli = Cli::parse();

    let result = match &cli {
        Cli::Tokenize { file } => cmd_tokenize(file),
        Cli::Parse { file } => cmd_parse(file),
        Cli::Check { file } => cmd_check(file),
        Cli::Build { file, output } => cmd_build(file, output.as_ref()),
        Cli::Repl => cmd_repl(),
    };

    if let Err(_msg) = result {
        std::process::exit(1);
    }
}

fn cmd_repl() -> Result<(), String> {
    eprintln!("AzuriteLang REPL (type 'exit' to quit)");
    let mut input = String::new();
    loop {
        input.clear();
        eprint!("> ");
        use std::io::Write;
        std::io::stdout().flush().ok();
        let read = std::io::stdin().read_line(&mut input);
        if read.is_err() || read.unwrap_or(0) == 0 { break; }
        let trimmed = input.trim();
        if trimmed == "exit" || trimmed.is_empty() { if trimmed == "exit" { break; } continue; }
        match Lexer::new(trimmed).tokenize() {
            Ok(tokens) => {
                let kinds: Vec<String> = tokens.iter().map(|t| t.kind.to_string()).collect();
                println!("  tokens: {}", kinds.join(" "));
                match Parser::new(tokens).parse_program() {
                    Ok(prog) => {
                        let mut checker = Checker::new();
                        match checker.check_program(&prog) {
                            Ok(()) => println!("  OK"),
                            Err(errs) => {
                                for err in &errs { eprintln!("  type error: {}", err.message); }
                            }
                        }
                    }
                    Err(e) => eprintln!("  parse error: {}", e.message),
                }
            }
            Err(e) => eprintln!("  lex error: {}", e),
        }
    }
    Ok(())
}

fn read_file(path: &PathBuf) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("cannot read {}: {}", path.display(), e))
}

fn resolve_module(source: &str, base_path: &Path) -> Result<Program, String> {
    let (mut parser, _tokens) = Parser::from_source(source).map_err(|e| e.to_string())?;
    let program = parser.parse_program().map_err(|e| e.to_string())?;
    let mut resolved = Vec::new();
    for stmt in program.statements {
        match stmt {
            Stmt::Import { path, .. } => {
                let import_path = base_path.parent().unwrap_or(Path::new(".")).join(&path);
                let import_source = read_file(&import_path.to_path_buf())?;
                let import_prog = resolve_module(&import_source, &import_path)?;
                resolved.extend(import_prog.statements);
            }
            other => resolved.push(other),
        }
    }
    Ok(Program { statements: resolved })
}

fn resolve_main(file: &Path) -> Result<(Program, String), String> {
    let source = read_file(&file.to_path_buf())?;
    let program = resolve_module(&source, file)?;
    Ok((program, source))
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
    let (program, _source) = resolve_main(file)?;
    println!("{:#?}", program);
    Ok(())
}

fn cmd_check(file: &PathBuf) -> Result<(), String> {
    let (program, source) = resolve_main(file)?;
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

fn cmd_build(file: &PathBuf, output: Option<&PathBuf>) -> Result<(), String> {
    let (program, _source) = resolve_main(file)?;

    #[cfg(feature = "llvm")]
    {
        let context = Context::create();
        let mut cg = CodeGen::new(&context, "azurite_program");
        cg.compile_program(&program).map_err(|e| e.to_string())?;

        let ll_path = file.with_extension("ll");
        cg.module().print_to_file(&ll_path)
            .map_err(|e| format!("cannot write .ll: {}", e))?;
        println!("LLVM IR: {}", ll_path.display());

        let clang_candidates = [
            "C:\\Program Files\\LLVM\\bin\\clang.exe",
            "D:\\Util\\LLVM\\bin\\clang.exe",
            "clang.exe",
        ];
        let clang = clang_candidates.iter().find(|p| Path::new(p).exists()).unwrap_or(&"clang.exe");
        let exe = output.map(|o| o.to_path_buf()).unwrap_or_else(|| file.with_extension("exe"));

        let mut cmd = std::process::Command::new(clang);
        cmd.args([&ll_path.to_string_lossy(), "-o", &exe.to_string_lossy()]);
        cmd.args(["-Wl,/defaultlib:msvcrt", "-Wl,/defaultlib:oldnames"]);
        match cmd.status() {
            Ok(s) if s.success() => println!("Executable: {}", exe.display()),
            _ => println!("LLVM IR generated. Install clang for .exe"),
        }
    }

    #[cfg(not(feature = "llvm"))]
    {
        let _ = (output, program);
        println!("LLVM backend not enabled. Use --features llvm");
    }

    Ok(())
}
