pub mod manifest;
pub mod resolver;

pub use manifest::{parse_manifest, DependencySpec, Manifest, Package};
pub use resolver::{find_dep_entry, load_lockfile, resolve_dependencies, save_lockfile, DepMap, LockEntry};
