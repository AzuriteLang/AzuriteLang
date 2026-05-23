use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::manifest::Manifest;

pub type DepMap = HashMap<String, PathBuf>;

pub fn resolve_dependencies(manifest: &Manifest, project_dir: &Path) -> Result<DepMap, String> {
    let cache_dir = get_cache_dir()?;
    let mut map = DepMap::new();

    for (name, dep) in &manifest.dependencies {
        let resolved = if let Some(path) = &dep.path {
            project_dir.join(path)
        } else if let Some(git_url) = &dep.git {
            let dep_cache = cache_dir.join(sanitize_name(name));
            if !dep_cache.exists() {
                eprintln!("  fetching {} from {} ...", name, git_url);
                let status = Command::new("git")
                    .args(["clone", "--depth", "1", git_url, &dep_cache.to_string_lossy()])
                    .status()
                    .map_err(|e| format!("failed to run git: {}", e))?;

                if !status.success() {
                    return Err(format!("failed to clone '{}' from {}", name, git_url));
                }
            }
            if let Some(rev) = &dep.rev {
                let status = Command::new("git")
                    .args(["checkout", rev])
                    .current_dir(&dep_cache)
                    .status()
                    .map_err(|e| format!("failed to run git checkout: {}", e))?;
                if !status.success() {
                    return Err(format!("failed to checkout rev '{}' for {}", rev, name));
                }
            }
            dep_cache
        } else {
            return Err(format!("dependency '{}' has no 'git' or 'path' field", name));
        };

        map.insert(name.clone(), resolved);
    }

    Ok(map)
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
    Err(format!(
        "no entry point found in dependency at {} (expected src/lib.az or main.az)",
        dep_path.display()
    ))
}

fn get_cache_dir() -> Result<PathBuf, String> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| "cannot determine home directory".to_string())?;

    let cache = Path::new(&home).join(".azurite").join("cache");
    std::fs::create_dir_all(&cache).map_err(|e| format!("cannot create cache dir: {}", e))?;
    Ok(cache)
}

fn sanitize_name(name: &str) -> String {
    name.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect()
}
