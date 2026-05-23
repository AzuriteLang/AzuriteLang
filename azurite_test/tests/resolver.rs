use azurite_resolver::parse_manifest;
use azurite_resolver::find_dep_entry;
use std::path::Path;

#[test]
fn test_parse_package_only() {
    let toml = r#"
[package]
name = "test-proj"
version = "1.2.3"
"#;
    let m = parse_manifest(toml).unwrap();
    assert_eq!(m.package.name, "test-proj");
    assert_eq!(m.package.version, "1.2.3");
    assert!(m.dependencies.is_empty());
}

#[test]
fn test_parse_git_dep() {
    let toml = r#"
[package]
name = "x"
version = "0.1.0"

[dependencies]
foo = { git = "https://github.com/azurite/foo" }
"#;
    let m = parse_manifest(toml).unwrap();
    let dep = &m.dependencies["foo"];
    assert_eq!(dep.git.as_deref(), Some("https://github.com/azurite/foo"));
    assert!(dep.path.is_none());
    assert!(dep.rev.is_none());
}

#[test]
fn test_parse_git_dep_with_rev() {
    let toml = r#"
[package]
name = "x"
version = "0.1.0"

[dependencies]
bar = { git = "https://github.com/azurite/bar", rev = "v2.0.0" }
"#;
    let m = parse_manifest(toml).unwrap();
    let dep = &m.dependencies["bar"];
    assert_eq!(dep.git.as_deref(), Some("https://github.com/azurite/bar"));
    assert_eq!(dep.rev.as_deref(), Some("v2.0.0"));
}

#[test]
fn test_parse_path_dep() {
    let toml = r#"
[package]
name = "x"
version = "0.1.0"

[dependencies]
local = { path = "../my-lib" }
"#;
    let m = parse_manifest(toml).unwrap();
    let dep = &m.dependencies["local"];
    assert_eq!(dep.path.as_deref(), Some("../my-lib"));
    assert!(dep.git.is_none());
}

#[test]
fn test_parse_multiple_deps() {
    let toml = r#"
[package]
name = "x"
version = "0.1.0"

[dependencies]
a = { git = "https://github.com/azurite/a" }
b = { path = "./libs/b" }
c = { git = "https://github.com/azurite/c", rev = "abc123" }
"#;
    let m = parse_manifest(toml).unwrap();
    assert_eq!(m.dependencies.len(), 3);
    assert_eq!(m.dependencies["a"].git.as_deref(), Some("https://github.com/azurite/a"));
    assert_eq!(m.dependencies["b"].path.as_deref(), Some("./libs/b"));
    assert_eq!(m.dependencies["c"].rev.as_deref(), Some("abc123"));
}

#[test]
fn test_parse_empty_manifest() {
    let toml = r#"
[package]
name = ""
version = ""
"#;
    let m = parse_manifest(toml).unwrap();
    assert_eq!(m.package.name, "");
    assert_eq!(m.package.version, "");
    assert!(m.dependencies.is_empty());
}

#[test]
fn test_parse_with_comments() {
    let toml = r#"
# This is a comment
[package]
name = "my-app"  # inline comment
version = "0.1.0"

[dependencies]
# string = { git = "https://github.com/azurite/string" }
math = { git = "https://github.com/azurite/math" }
"#;
    let m = parse_manifest(toml).unwrap();
    assert_eq!(m.package.name, "my-app");
    assert_eq!(m.dependencies.len(), 1);
    assert!(m.dependencies.contains_key("math"));
}

#[test]
fn test_find_dep_entry_missing_dir() {
    let err = find_dep_entry(Path::new("C:\\non_existent_dir_azurite_test_12345"));
    assert!(err.is_err());
    assert!(err.unwrap_err().contains("entry point"));
}

#[test]
fn test_parse_invalid_unquoted_string() {
    let toml = r#"
[package]
name = unquoted
"#;
    let err = parse_manifest(toml);
    assert!(err.is_err());
}

#[test]
fn test_parse_dep_without_git_or_path() {
    let toml = r#"
[package]
name = "x"
version = "0.1.0"

[dependencies]
foo = { rev = "abc" }
"#;
    let m = parse_manifest(toml).unwrap();
    let dep = &m.dependencies["foo"];
    assert!(dep.git.is_none());
    assert!(dep.path.is_none());
    assert_eq!(dep.rev.as_deref(), Some("abc"));
}
