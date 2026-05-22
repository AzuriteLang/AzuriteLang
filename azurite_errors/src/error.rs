use azurite_lexer::Span;

#[derive(Debug, Clone)]
pub enum ErrorKind {
    Lex,
    Parse,
    TypeError,
    Semantic,
}

impl std::fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorKind::Lex => write!(f, "lex error"),
            ErrorKind::Parse => write!(f, "parse error"),
            ErrorKind::TypeError => write!(f, "type error"),
            ErrorKind::Semantic => write!(f, "semantic error"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AzError {
    pub kind: ErrorKind,
    pub span: Span,
    pub message: String,
    pub help: Option<String>,
}

impl AzError {
    pub fn new(kind: ErrorKind, span: Span, message: impl Into<String>) -> Self {
        Self {
            kind,
            span,
            message: message.into(),
            help: None,
        }
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }
}

impl std::fmt::Display for AzError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} at {}:{}: {}", self.kind, self.span.line, self.span.column, self.message)
    }
}

pub type Result<T> = std::result::Result<T, AzError>;
