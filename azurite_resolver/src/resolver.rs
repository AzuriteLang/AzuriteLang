use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::manifest::Manifest;

pub type DepMap = HashMap<String, PathBuf>;

#[derive(Debug, Clone)]
pub struct LockEntry {
    pub name: String,
    pub git: Option<String>,
    pub path: Option<String>,
    pub commit: Option<String>,
    pub rev: Option<String>,
}

pub fn load_lockfile(project_dir: &Path) -> Result<Vec<LockEntry>, String> {
    let lock_path = project_dir.join("azurite.lock");
    if !lock_path.exists() { return Ok(Vec::new()); }
    let content = fs::read_to_string(&lock_path)
        .map_err(|e| format!("cannot read {}: {}", lock_path.display(), e))?;
    let mut entries = Vec::new();
    let table: toml::Table = toml::from_str(&content).map_err(|e| format!("invalid azurite.lock: {}", e))?;
    // Parse as array of tables or numbered entries
    for i in 0.. {
        let key = format!("dependency.{}", i);
        let dep = match table.get(&key) {
            Some(toml::Value::Table(t)) => t,
            _ => break,
        };
        entries.push(LockEntry {
            name: dep.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            git: dep.get("git").and_then(|v| v.as_str()).map(String::from),
            path: dep.get("path").and_then(|v| v.as_str()).map(String::from),
            commit: dep.get("commit").and_then(|v| v.as_str()).map(String::from),
            rev: dep.get("rev").and_then(|v| v.as_str()).map(String::from),
        });
    }
    Ok(entries)
}

pub fn save_lockfile(project_dir: &Path, entries: &[LockEntry]) -> Result<(), String> {
    let mut out = String::new();
    out.push_str("# Azurite lockfile\n");
    for (i, entry) in entries.iter().enumerate() {
        out.push_str(&format!("\n[dependency.{}]\n", i));
        out.push_str(&format!("name = \"{}\"\n", entry.name));
        if let Some(ref g) = entry.git { out.push_str(&format!("git = \"{}\"\n", g)); }
        if let Some(ref p) = entry.path { out.push_str(&format!("path = \"{}\"\n", p)); }
        if let Some(ref c) = entry.commit { out.push_str(&format!("commit = \"{}\"\n", c)); }
        if let Some(ref r) = entry.rev { out.push_str(&format!("rev = \"{}\"\n", r)); }
    }
    let lock_path = project_dir.join("azurite.lock");
    fs::write(&lock_path, &out)
        .map_err(|e| format!("cannot write {}: {}", lock_path.display(), e))
}

fn compute_commit(cache_path: &Path) -> Result<String, String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(cache_path)
        .output()
        .map_err(|e| format!("git rev-parse failed: {}", e))?;
    if !output.status.success() {
        return Err("git rev-parse returned non-zero exit".to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn verify_integrity(name: &str, cache_path: &Path, expected_commit: &str) -> Result<(), String> {
    let actual = compute_commit(cache_path)?;
    if actual != expected_commit {
        return Err(format!(
            "integrity check failed for '{}': expected commit {}, got {}. Run --update to refetch.",
            name, expected_commit, actual
        ));
    }
    Ok(())
}

pub fn resolve_dependencies(
    manifest: &Manifest,
    project_dir: &Path,
    force_update: bool,
) -> Result<(DepMap, Vec<LockEntry>), String> {
    let cache_dir = get_cache_dir()?;
    let mut map = DepMap::new();
    let lock = load_lockfile(project_dir)?;
    let mut lock_entries: Vec<LockEntry> = Vec::new();

    for (name, dep) in &manifest.dependencies {
        let (resolved, entry) = if let Some(path) = &dep.path {
            let resolved = project_dir.join(path);
            if !resolved.exists() {
                return Err(format!(
                    "dependency '{}': path '{}' does not exist",
                    name, resolved.display()
                ));
            }
            let entry = LockEntry {
                name: name.clone(),
                git: None,
                path: Some(path.clone()),
                commit: None,
                rev: None,
            };
            (resolved, entry)
        } else if let Some(git_url) = &dep.git {
            let dep_cache = cache_dir.join(sanitize_name(name));

            // Check lockfile for expected commit
            let expected_commit = lock.iter()
                .find(|e| e.name == *name)
                .and_then(|e| e.commit.clone());

            let needs_clone = !dep_cache.exists();

            if needs_clone {
                eprintln!("  fetching {} from {} ...", name, git_url);
                let status = Command::new("git")
                    .args(["clone", "--depth", "1", git_url, &dep_cache.to_string_lossy()])
                    .status()
                    .map_err(|e| format!("failed to run git: {}", e))?;
                if !status.success() {
                    // Clean up failed clone
                    let _ = fs::remove_dir_all(&dep_cache);
                    return Err(format!(
                        "failed to clone dependency '{}' from {}.\n\
                         Check that the URL is correct and the repository exists.",
                        name, git_url
                    ));
                }
            } else if force_update {
                eprintln!("  updating {} ...", name);
                let status = Command::new("git")
                    .args(["fetch", "origin"])
                    .current_dir(&dep_cache)
                    .status()
                    .map_err(|e| format!("failed to run git fetch: {}", e))?;
                if !status.success() {
                    return Err(format!("failed to fetch updates for '{}'", name));
                }
                let status = Command::new("git")
                    .args(["reset", "--hard", "origin/HEAD"])
                    .current_dir(&dep_cache)
                    .status()
                    .map_err(|e| format!("failed to reset: {}", e))?;
                if !status.success() {
                    return Err(format!("failed to reset '{}' to origin/HEAD", name));
                }
            } else if let Some(ref expected) = expected_commit {
                // Verify integrity
                if let Err(e) = verify_integrity(name, &dep_cache, expected) {
                    eprintln!("  warning: {}", e);
                    eprintln!("  hint: use --update to refresh dependencies");
                }
            }

            // Checkout specific rev if specified
            if let Some(rev) = &dep.rev {
                let status = Command::new("git")
                    .args(["checkout", rev])
                    .current_dir(&dep_cache)
                    .status()
                    .map_err(|e| format!("failed to run git checkout: {}", e))?;
                if !status.success() {
                    return Err(format!(
                        "failed to checkout revision '{}' for '{}'. The revision may not exist.",
                        rev, name
                    ));
                }
            }

            let commit = compute_commit(&dep_cache).ok();
            let entry = LockEntry {
                name: name.clone(),
                git: Some(git_url.clone()),
                path: None,
                commit,
                rev: dep.rev.clone(),
            };
            (dep_cache, entry)
        } else {
            return Err(format!(
                "dependency '{}' has no 'git' or 'path' field.\n\
                 Example: {} = {{ git = \"https://github.com/AzuriteLang/string\" }}",
                name, name
            ));
        };

        // Store the dependency directory (resolve_module will find the entry point)
        map.insert(name.clone(), resolved.clone());
        lock_entries.push(entry);
    }

    // Save lockfile
    if let Err(e) = save_lockfile(project_dir, &lock_entries) {
        eprintln!("  warning: could not save lockfile: {}", e);
    }

    Ok((map, lock_entries))
}

pub fn find_dep_entry(dep_path: &Path) -> Result<PathBuf, String> {
    let candidates = [
        dep_path.join("src").join("lib.az"),
        dep_path.join("main.az"),
        dep_path.join("src").join("main.az"),
    ];
    for c in &candidates {
        if c.exists() {
            return Ok(c.clone());
        }
    }
    let display = dep_path.display();
    Err(format!(
        "dependency at {} has no entry point.\n\
         Expected one of:\n\
         - {}/src/lib.az\n\
         - {}/main.az",
        display, display, display
    ))
}

fn get_cache_dir() -> Result<PathBuf, String> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| "cannot determine home directory. Set $HOME or %USERPROFILE%.".to_string())?;

    let cache = Path::new(&home).join(".azurite").join("cache");
    fs::create_dir_all(&cache).map_err(|e| format!("cannot create cache directory at {}: {}", cache.display(), e))?;
    Ok(cache)
}

fn sanitize_name(name: &str) -> String {
    name.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect()
}
