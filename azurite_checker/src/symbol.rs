use crate::types::Type;

#[derive(Debug, Clone)]
pub enum SymbolKind {
    Variable,
    Function,
}

#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub type_: Type,
}

#[derive(Debug)]
pub struct Scope {
    symbols: Vec<Vec<(String, Symbol)>>,
}

impl Scope {
    pub fn new() -> Self {
        Self {
            symbols: vec![Vec::new()],
        }
    }

    pub fn push(&mut self) {
        self.symbols.push(Vec::new());
    }

    pub fn pop(&mut self) {
        self.symbols.pop();
    }

    pub fn insert(&mut self, name: &str, symbol: Symbol) -> Result<(), String> {
        let current = self.symbols.last_mut().unwrap();
        if current.iter().any(|(n, _)| n == name) {
            return Err(format!("'{}' is already defined in this scope", name));
        }
        current.push((name.to_string(), symbol));
        Ok(())
    }

    pub fn lookup(&self, name: &str) -> Option<&Symbol> {
        for scope in self.symbols.iter().rev() {
            if let Some((_, sym)) = scope.iter().find(|(n, _)| n == name) {
                return Some(sym);
            }
        }
        None
    }

    pub fn lookup_current(&self, name: &str) -> Option<&Symbol> {
        let current = self.symbols.last().unwrap();
        current.iter().find(|(n, _)| n == name).map(|(_, s)| s)
    }
}
