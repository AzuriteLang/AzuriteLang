use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Manifest {
    pub package: Package,
    pub dependencies: HashMap<String, DependencySpec>,
}

#[derive(Debug, Clone)]
pub struct Package {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone)]
pub struct DependencySpec {
    pub git: Option<String>,
    pub path: Option<String>,
    pub rev: Option<String>,
}

pub fn parse_manifest(content: &str) -> Result<Manifest, String> {
    let mut package = Package { name: String::new(), version: String::new() };
    let mut dependencies = HashMap::new();
    let mut section = String::new();

    for line in content.lines() {
        let stripped = strip_comment(line);
        let trimmed = stripped.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.starts_with('[') {
            if let Some(end) = trimmed.find(']') {
                section = trimmed[1..end].trim().to_string();
            }
            continue;
        }

        if let Some(eq_pos) = trimmed.find('=') {
            let key = trimmed[..eq_pos].trim().to_string();
            let value = trimmed[eq_pos + 1..].trim();

            match section.as_str() {
                "package" => match key.as_str() {
                    "name" => package.name = parse_string(value)?,
                    "version" => package.version = parse_string(value)?,
                    _ => {}
                },
                "dependencies" => {
                    let dep_name = key;
                    if value.starts_with('{') {
                        let inner = value.trim_start_matches('{').trim_end_matches('}').trim();
                        let mut git = None;
                        let mut path = None;
                        let mut rev = None;
                        for part in split_inline_table(inner) {
                            if let Some(eq2) = part.find('=') {
                                let k = part[..eq2].trim();
                                let v = part[eq2 + 1..].trim();
                                match k {
                                    "git" => git = Some(parse_string(v)?),
                                    "path" => path = Some(parse_string(v)?),
                                    "rev" => rev = Some(parse_string(v)?),
                                    _ => {}
                                }
                            }
                        }
                        dependencies.insert(dep_name, DependencySpec { git, path, rev });
                    }
                }
                _ => {}
            }
        }
    }

    Ok(Manifest { package, dependencies })
}

fn strip_comment(line: &str) -> String {
    let mut in_string = false;
    let mut prev_was_backslash = false;
    for (i, ch) in line.char_indices() {
        if ch == '"' && !prev_was_backslash {
            in_string = !in_string;
        }
        prev_was_backslash = ch == '\\' && !prev_was_backslash;
        if ch == '#' && !in_string {
            return line[..i].to_string();
        }
    }
    line.to_string()
}

fn parse_string(s: &str) -> Result<String, String> {
    let s = s.trim();
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        Ok(s[1..s.len() - 1].to_string())
    } else {
        Err(format!("expected quoted string, got: {}", s))
    }
}

fn split_inline_table(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0;
    let mut start = 0;
    for (i, ch) in s.char_indices() {
        match ch {
            '{' | '[' => depth += 1,
            '}' | ']' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(s[start..i].trim().to_string());
                start = i + 1;
            }
            _ => {}
        }
    }
    let last = s[start..].trim().to_string();
    if !last.is_empty() {
        parts.push(last);
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_full() {
        let toml = r#"
[package]
name = "my-project"
version = "0.1.0"

[dependencies]
string = { git = "https://github.com/azurite/string" }
math = { git = "https://github.com/azurite/math", rev = "v0.2.0" }
local = { path = "../my-lib" }
"#;
        let m = parse_manifest(toml).unwrap();
        assert_eq!(m.package.name, "my-project");
        assert_eq!(m.package.version, "0.1.0");
        assert_eq!(m.dependencies.len(), 3);
        assert_eq!(m.dependencies["string"].git.as_deref(), Some("https://github.com/azurite/string"));
        assert_eq!(m.dependencies["math"].rev.as_deref(), Some("v0.2.0"));
        assert_eq!(m.dependencies["local"].path.as_deref(), Some("../my-lib"));
    }
}
