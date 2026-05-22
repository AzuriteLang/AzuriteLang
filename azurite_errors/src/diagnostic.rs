use crate::error::{AzError, ErrorKind};

const RED: &str = "\x1b[31m";
const CYAN: &str = "\x1b[36m";
const YELLOW: &str = "\x1b[33m";
const BOLD: &str = "\x1b[1m";
const RESET: &str = "\x1b[0m";

pub struct Diagnostic;

impl Diagnostic {
    pub fn print(source: &str, file: &str, error: &AzError) {
        let prefix = match error.kind {
            ErrorKind::Lex => format!("{}error{}", RED, RESET),
            ErrorKind::Parse => format!("{}parse error{}", RED, RESET),
            ErrorKind::TypeError => format!("{}type error{}", RED, RESET),
            ErrorKind::Semantic => format!("{}semantic error{}", RED, RESET),
        };

        let loc = format!("{}-->{} {}:{}:{}", CYAN, RESET, file, error.span.line, error.span.column);
        eprintln!("{}{}:{}{} {}", BOLD, prefix, RESET, "", error.message);
        eprintln!("  {}", loc);

        let lines: Vec<&str> = source.lines().collect();
        if error.span.line > 0 && error.span.line <= lines.len() {
            let line_idx = error.span.line - 1;
            let line = lines[line_idx];
            let col = error.span.column.saturating_sub(1);

            eprintln!("{} |{}", CYAN, RESET);
            let line_num = format!("{:>3}", error.span.line);
            eprintln!("{} {}|{} {}", CYAN, line_num, RESET, line);

            let mut caret = String::new();
            // indent to column
            for _ in 0..col {
                caret.push(' ');
            }
            let span_len = std::cmp::max(error.span.end - error.span.start, 1);
            for idx in 0..span_len {
                if idx == 0 {
                    caret.push_str(&format!("{}^", RED));
                } else {
                    caret.push_str(&format!("{}~", RED));
                }
            }
            eprintln!("  {} |{} {}", CYAN, RESET, caret);

            if let Some(ref help) = error.help {
                eprintln!("  {} ={} {}: {}", YELLOW, RESET, BOLD, RESET);
                eprintln!("  {} ={} {}", YELLOW, RESET, help);
            }
        }
        eprintln!();
    }
}
