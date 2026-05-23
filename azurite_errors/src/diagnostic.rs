use crate::error::{AzError, ErrorKind};

const RED: &str = "\x1b[31m";
const CYAN: &str = "\x1b[36m";
const YELLOW: &str = "\x1b[33m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const RESET: &str = "\x1b[0m";

pub struct Diagnostic;

impl Diagnostic {
    fn error_code(kind: &ErrorKind) -> &str {
        match kind {
            ErrorKind::Lex => "E0001",
            ErrorKind::Parse => "E0002",
            ErrorKind::TypeError => "E0003",
            ErrorKind::Semantic => "E0004",
        }
    }

    fn error_label(kind: &ErrorKind) -> &str {
        match kind {
            ErrorKind::Lex => "lexical error",
            ErrorKind::Parse => "parse error",
            ErrorKind::TypeError => "type error",
            ErrorKind::Semantic => "semantic error",
        }
    }

    pub fn print(source: &str, file: &str, error: &AzError) {
        let code = Self::error_code(&error.kind);
        let _label = Self::error_label(&error.kind);
        let span = error.span;

        // Header: error code + message
        let header = format!("{}{}{} {} {}[{}{}{}]{}",
            RED, BOLD, "error", RESET,
            BOLD, CYAN, code, RESET, BOLD,
        );
        eprintln!("{} {}", header, error.message);

        // Location
        if span.line > 0 && span.column > 0 {
            eprintln!("  {}{}-->{} {}:{}:{}", CYAN, DIM, RESET, file, span.line, span.column);

            let lines: Vec<&str> = source.lines().collect();
            if span.line <= lines.len() {
                let line_idx = span.line - 1;
                let line = lines[line_idx];
                let col = span.column.saturating_sub(1);
                let end = std::cmp::min(span.end.saturating_sub(1), line.len());

                // Line number gutter
                let line_num = format!("{:>3}", span.line);
                eprintln!(" {} {}{} {}|{} {}", DIM, CYAN, line_num, RESET, DIM, RESET);

                // The source line
                eprintln!(" {} {}|{} {}", CYAN, line_num, RESET, line);

                // Underline with carets
                if end >= col {
                    let mut caret = String::new();
                    for _ in 0..col {
                        caret.push(' ');
                    }
                    let width = std::cmp::max(end - col, 1);
                    caret.push_str(&format!("{}{}", RED, "^".repeat(width)));
                    caret.push_str(RESET);
                    eprintln!(" {} {}|{} {}", CYAN, line_num, RESET, caret);
                }
            }
        } else {
            eprintln!("  {}{}-->{} {}", CYAN, DIM, RESET, file);
        }

        // Help message
        if let Some(ref help) = error.help {
            eprintln!("  {}{}={}{} {}", YELLOW, DIM, RESET, DIM, help);
        }
        eprintln!();
    }
}
