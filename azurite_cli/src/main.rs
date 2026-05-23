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
use azurite_resolver::{find_dep_entry, parse_manifest, resolve_dependencies, DepMap};

#[cfg(feature = "llvm")]
use inkwell::context::Context;

#[derive(ClapParser)]
#[command(name = "azurite", about = "AzuriteLang compiler")]
enum Cli {
    #[command(about = "Type-check source and report errors")]
    Check { file: PathBuf },
    #[command(about = "Compile source to executable")]
    Build {
        file: PathBuf,
        #[arg(short, long, help = "Output file path")]
        output: Option<PathBuf>,
    },
    #[command(about = "Interactive REPL")]
    Repl,
    #[command(about = "Initialize a new Azurite project with azurite.toml")]
    Init {
        #[arg(default_value = ".")]
        dir: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();

    let result = match &cli {
        Cli::Check { file } => cmd_check(file),
        Cli::Build { file, output } => cmd_build(file, output.as_ref()),
        Cli::Repl => cmd_repl(),
        Cli::Init { dir } => cmd_init(dir),
    };

    if let Err(msg) = result {
        eprintln!("{}", msg);
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

fn cmd_init(dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("cannot create directory: {}", e))?;
    let manifest_path = dir.join("azurite.toml");
    if manifest_path.exists() {
        return Err(format!("azurite.toml already exists in {}", dir.display()));
    }
    let default_name = dir.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("azurite_project");
    let content = format!(
        r#"[package]
name = "{}"
version = "0.1.0"

[dependencies]
# string = {{ git = "https://github.com/azurite/string" }}
# math  = {{ git = "https://github.com/azurite/math" }}
"#,
        default_name
    );
    fs::write(&manifest_path, &content)
        .map_err(|e| format!("cannot write azurite.toml: {}", e))?;
    println!("Created {}", manifest_path.display());
    Ok(())
}

fn read_file(path: &PathBuf) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("cannot read {}: {}", path.display(), e))
}

fn find_manifest(start: &Path) -> Option<PathBuf> {
    let mut current = Some(start.to_path_buf());
    while let Some(dir) = current {
        let manifest = dir.join("azurite.toml");
        if manifest.exists() {
            return Some(manifest);
        }
        current = dir.parent().map(|p| p.to_path_buf());
    }
    None
}

fn resolve_module(source: &str, base_path: &Path, deps: &DepMap) -> Result<Program, String> {
    let (mut parser, _tokens) = Parser::from_source(source).map_err(|e| e.to_string())?;
    let program = parser.parse_program().map_err(|e| e.to_string())?;
    let mut resolved = Vec::new();
    for stmt in program.statements {
        match stmt {
            Stmt::Import { path, .. } => {
                if let Some(dep_path) = deps.get(&path) {
                    let entry = find_dep_entry(dep_path)?;
                    let import_source = read_file(&entry)?;
                    let import_prog = resolve_module(&import_source, &entry, deps)?;
                    resolved.extend(import_prog.statements);
                } else {
                    let import_path = base_path.parent().unwrap_or(Path::new(".")).join(&path);
                    let import_source = read_file(&import_path.to_path_buf())?;
                    let import_prog = resolve_module(&import_source, &import_path, deps)?;
                    resolved.extend(import_prog.statements);
                }
            }
            other => resolved.push(other),
        }
    }
    Ok(Program { statements: resolved })
}

fn resolve_main(file: &Path) -> Result<(Program, String), String> {
    let deps = if let Some(manifest_path) = find_manifest(file) {
        let content = read_file(&manifest_path)?;
        let manifest = parse_manifest(&content)?;
        let project_dir = manifest_path.parent().unwrap_or(Path::new("."));
        eprintln!("Loaded {}", manifest_path.display());
        resolve_dependencies(&manifest, project_dir)?
    } else {
        DepMap::new()
    };

    let source = read_file(&file.to_path_buf())?;
    let program = resolve_module(&source, file, &deps)?;
    Ok((program, source))
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
        let clang_ok = match cmd.status() {
            Ok(s) if s.success() => { std::fs::remove_file(&ll_path).ok(); true }
            _ => false,
        };
        if clang_ok {
            println!("Executable: {}", exe.display());
        } else {
            println!("LLVM IR generated. Install clang for .exe");
        }
    }

    #[cfg(not(feature = "llvm"))]
    {
        let _ = (output, program);
        println!("LLVM backend not enabled. Use --features llvm");
    }

    Ok(())
}
