pub mod manifest;
pub mod resolver;

pub use manifest::{parse_manifest, DependencySpec, Manifest, Package};
pub use resolver::{find_dep_entry, resolve_dependencies, DepMap};
