use crate::{Project, Result, SnpmConfig, SnpmError};

use std::fs;

/// The global install space is an ordinary snpm project living at
/// `config.global_dir()`: a managed `package.json` whose dependencies
/// are the globally installed packages, plus the usual
/// `snpm-lock.yaml` / `node_modules` / `.snpm` produced by the standard
/// install pipeline. This gives global installs the full resolver,
/// store, virtual-store linking, and hot-path behavior for free —
/// including transitive dependencies, which a package copied in
/// isolation would be missing at runtime.
const GLOBAL_MANIFEST_TEMPLATE: &str =
    "{\n  \"name\": \"snpm-global\",\n  \"private\": true,\n  \"dependencies\": {}\n}\n";

pub(super) fn global_project(config: &SnpmConfig) -> Result<Project> {
    let global_dir = config.global_dir();
    fs::create_dir_all(&global_dir).map_err(|source| SnpmError::WriteFile {
        path: global_dir.clone(),
        source,
    })?;

    let manifest_path = global_dir.join("package.json");
    if !manifest_path.is_file() {
        fs::write(&manifest_path, GLOBAL_MANIFEST_TEMPLATE).map_err(|source| {
            SnpmError::WriteFile {
                path: manifest_path.clone(),
                source,
            }
        })?;
    }

    Project::from_manifest_path(manifest_path)
}
