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
use rustyline::config::Configurer;

#[cfg(feature = "llvm")]
use inkwell::context::Context;

#[derive(ClapParser)]
#[command(name = "azurite", about = "AzuriteLang compiler")]
enum Cli {
    #[command(about = "Type-check source and report errors")]
    Check {
        file: PathBuf,
        #[arg(long, help = "Force update dependencies")]
        update: bool,
    },
    #[command(about = "Compile source to executable")]
    Build {
        file: PathBuf,
        #[arg(short, long, help = "Output file path")]
        output: Option<PathBuf>,
        #[arg(long, help = "Keep .ll file after compilation")]
        keep_ll: bool,
        #[arg(long, help = "Force update dependencies")]
        update: bool,
    },
    #[command(about = "Interactive REPL")]
    Repl,
    #[command(about = "Initialize a new Azurite project with azurite.toml")]
    Init {
        #[arg(default_value = ".")]
        dir: PathBuf,
    },
    #[command(about = "Install a dependency (from registry or custom)")]
    Install {
        name: String,
        #[arg(long, help = "Git URL")]
        git: Option<String>,
        #[arg(long, help = "Local path")]
        path: Option<String>,
        #[arg(long, help = "Git revision")]
        rev: Option<String>,
    },
    #[command(about = "Update all dependencies to latest")]
    Update,
}

fn main() {
    let cli = Cli::parse();

    let result = match &cli {
        Cli::Check { file, update } => cmd_check(file, *update),
        Cli::Build { file, output, keep_ll, update } => cmd_build(file, output.as_ref(), *keep_ll, *update),
        Cli::Repl => cmd_repl(),
        Cli::Init { dir } => cmd_init(dir),
        Cli::Install { name, git, path, rev } => cmd_install(name, git.as_deref(), path.as_deref(), rev.as_deref()),
        Cli::Update => cmd_update(),
    };

    if let Err(msg) = result {
        eprintln!("{}", msg);
        std::process::exit(1);
    }
}

#[cfg(all(feature = "llvm"))]
fn repl_execute(source: &str, context: &Context) -> Result<(), String> {
    use inkwell::execution_engine::JitFunction;
    use azurite_codegen::CodeGen;

    let main_az = format!("func main() {{\n{}\n}}", source);
    let (mut parser, _tokens) = Parser::from_source(&main_az).map_err(|e| e.to_string())?;
    let program = parser.parse_program().map_err(|e| e.to_string())?;

    let mut checker = Checker::new();
    checker.check_program(&program).map_err(|errs| {
        errs.iter().map(|e| e.message.clone()).collect::<Vec<_>>().join("\n")
    })?;

    let mut cg = CodeGen::new(context, "repl_module");
    cg.compile_program(&program).map_err(|e| e.to_string())?;

    let ee = cg.module().create_jit_execution_engine(inkwell::OptimizationLevel::None)
        .map_err(|e| format!("JIT error: {}", e))?;
    unsafe {
        let func: JitFunction<unsafe extern "C" fn()> = ee.get_function("main")
            .map_err(|e| format!("JIT get_function: {}", e))?;
        func.call();
    }

    Ok(())
}

fn cmd_repl() -> Result<(), String> {
    let completions = vec![
        "let", "func", "if", "else", "while", "for", "in", "match", "return",
        "break", "continue", "import", "class", "enum", "true", "false", "null",
        "and", "or", "not", "self", "super", "int", "float", "string", "bool", "void",
        "print", "len", "chr", "sqrt", "abs", "int", "float",
        "sin", "cos", "tan", "asin", "acos", "atan", "atan2",
        "sinh", "cosh", "tanh", "exp", "expm1", "log", "log2", "log10",
        "pow", "hypot", "fmod", "copysign", "floor", "ceil",
        "char_at", "read", "input", "exit", "rand", "srand",
    ];

    struct AzCompleter(Vec<String>);
    impl rustyline::completion::Completer for AzCompleter {
        type Candidate = String;
        fn complete(&self, line: &str, pos: usize, _ctx: &rustyline::Context<'_>) -> rustyline::Result<(usize, Vec<String>)> {
            let prefix = &line[..pos];
            let word = prefix.split(|c: char| !c.is_alphanumeric() && c != '_').last().unwrap_or("");
            let matches: Vec<String> = self.0.iter()
                .filter(|c| c.starts_with(word))
                .cloned()
                .collect();
            let start = pos - word.len();
            Ok((start, matches))
        }
    }
    impl rustyline::hint::Hinter for AzCompleter { type Hint = String; fn hint(&self, _: &str, _: usize, _: &rustyline::Context<'_>) -> Option<String> { None } }
    impl rustyline::highlight::Highlighter for AzCompleter {}
    impl rustyline::validate::Validator for AzCompleter {}
    impl rustyline::Helper for AzCompleter {}

    let mut rl = rustyline::Editor::<AzCompleter, _>::new().map_err(|e| e.to_string())?;
    rl.set_max_history_size(500).ok();
    rl.set_helper(Some(AzCompleter(completions.iter().map(|s| s.to_string()).collect())));

    eprintln!("AzuriteLang REPL (Ctrl+C or 'exit' to quit)");

    #[cfg(feature = "llvm")]
    let context = Context::create();

    loop {
        let read = rl.readline("> ");
        match read {
            Ok(line) => {
                let trimmed = line.trim();
                if trimmed == "exit" || trimmed.is_empty() { if trimmed == "exit" { break; } continue; }
                rl.add_history_entry(&line).map_err(|e| e.to_string())?;

                match Lexer::new(trimmed).tokenize() {
                    Ok(tokens) => {
                        match Parser::new(tokens).parse_program() {
                            Ok(prog) => {
                                let mut checker = Checker::new();
                                match checker.check_program(&prog) {
                                    Ok(()) => {
                                        #[cfg(feature = "llvm")]
                                        if let Err(e) = repl_execute(trimmed, &context) {
                                            eprintln!("  error: {}", e);
                                        }
                                    }
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
            Err(rustyline::error::ReadlineError::Interrupted) => {
                eprintln!("exit");
                break;
            }
            Err(_) => break,
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

fn registry_url(name: &str) -> Option<String> {
    let known: &[(&str, &str)] = &[
        ("string", "https://github.com/AzuriteLang/string"),
        ("math", "https://github.com/AzuriteLang/math"),
        ("random", "https://github.com/AzuriteLang/random"),
        ("color", "https://github.com/AzuriteLang/color"),
        ("system", "https://github.com/AzuriteLang/system"),
    ];
    known.iter().find(|(n, _)| *n == name).map(|(_, u)| u.to_string())
}

fn cmd_install(name: &str, git: Option<&str>, path: Option<&str>, rev: Option<&str>) -> Result<(), String> {
    let cwd = std::env::current_dir().map_err(|e| format!("cannot get cwd: {}", e))?;
    let manifest_path = find_manifest(&cwd).unwrap_or_else(|| cwd.join("azurite.toml"));
    let mut content = if manifest_path.exists() {
        fs::read_to_string(&manifest_path).map_err(|e| format!("cannot read {}: {}", manifest_path.display(), e))?
    } else {
        let dir_name = cwd.file_name().and_then(|n| n.to_str()).unwrap_or("project");
        format!(
            r#"[package]
name = "{}"
version = "0.1.0"

[dependencies]
"#,
            dir_name
        )
    };

    let git_url = match (git, path) {
        (Some(url), _) => url.to_string(),
        (None, Some(_)) => String::new(),
        (None, None) => registry_url(name)
            .ok_or_else(|| format!("unknown package '{}'. Use --git <url> or --path <path>", name))?,
    };

    let dep_line = if let Some(p) = path {
        match rev {
            Some(r) => format!("{} = {{ path = \"{}\", rev = \"{}\" }}", name, p, r),
            None => format!("{} = {{ path = \"{}\" }}", name, p),
        }
    } else {
        match rev {
            Some(r) => format!("{} = {{ git = \"{}\", rev = \"{}\" }}", name, git_url, r),
            None => format!("{} = {{ git = \"{}\" }}", name, git_url),
        }
    };

    if content.lines().any(|l| l.trim().starts_with(&format!("{} =", name))) {
        return Err(format!("dependency '{}' already exists in azurite.toml", name));
    }

    if let Some(deps_pos) = content.find("[dependencies]") {
        let after = &content[deps_pos + 15..];
        let insert_pos = if let Some(last_dep) = after.rfind('\n') {
            deps_pos + 15 + last_dep + 1
        } else {
            content.len()
        };
        let indent = if content[insert_pos..].trim().is_empty() { "\n" } else { "\n" };
        content.insert_str(insert_pos, &format!("{}{}", indent, dep_line));
    } else {
        content.push_str(&format!("\n[dependencies]\n{}\n", dep_line));
    }

    fs::write(&manifest_path, &content)
        .map_err(|e| format!("cannot write {}: {}", manifest_path.display(), e))?;
    println!("Added '{}' to {}", name, manifest_path.display());
    Ok(())
}

fn cmd_update() -> Result<(), String> {
    let cwd = std::env::current_dir().map_err(|e| format!("cannot get cwd: {}", e))?;
    let manifest_path = find_manifest(&cwd).ok_or_else(|| "no azurite.toml found in current directory".to_string())?;
    let content = read_file(&manifest_path)?;
    let manifest = parse_manifest(&content)?;
    let project_dir = manifest_path.parent().unwrap();
    eprintln!("Updating dependencies in {}", manifest_path.display());
    resolve_dependencies(&manifest, project_dir, true)?;
    println!("All dependencies up to date.");
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

fn resolve_main(file: &Path, force_update: bool) -> Result<(Program, String), String> {
    let deps = if let Some(manifest_path) = find_manifest(file) {
        let content = read_file(&manifest_path)?;
        let manifest = parse_manifest(&content)?;
        let project_dir = manifest_path.parent().unwrap_or(Path::new("."));
        eprintln!("Loaded {}", manifest_path.display());
        resolve_dependencies(&manifest, project_dir, force_update)?.0
    } else {
        DepMap::new()
    };

    let source = read_file(&file.to_path_buf())?;
    let program = resolve_module(&source, file, &deps)?;
    Ok((program, source))
}

fn cmd_check(file: &PathBuf, force_update: bool) -> Result<(), String> {
    let (program, source) = resolve_main(file, force_update)?;
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

fn cmd_build(file: &PathBuf, output: Option<&PathBuf>, keep_ll: bool, force_update: bool) -> Result<(), String> {
    let (program, source) = resolve_main(file, force_update)?;

    let mut checker = Checker::new();
    if let Err(errors) = checker.check_program(&program) {
        for err in &errors {
            Diagnostic::print(&source, &file.to_string_lossy(), err);
        }
        return Err(format!("{} type error(s) found", errors.len()));
    }

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
        cmd.args(["-Wl,/defaultlib:msvcrt", "-Wl,/defaultlib:oldnames", "-Wl,/defaultlib:legacy_stdio_definitions"]);
        let clang_ok = match cmd.status() {
            Ok(s) if s.success() => { if !keep_ll { std::fs::remove_file(&ll_path).ok(); } true }
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
