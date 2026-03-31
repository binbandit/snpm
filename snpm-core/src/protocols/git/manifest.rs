use crate::project::Manifest;
use crate::registry::RegistryPackage;
use crate::{Result, SnpmError};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

pub(super) fn load_registry_package(repo_dir: &Path, url: &str) -> Result<RegistryPackage> {
    let manifest_path = repo_dir.join("package.json");
    if !manifest_path.exists() {
        return Err(SnpmError::ResolutionFailed {
            name: url.to_string(),
            range: "latest".to_string(),
            reason: "package.json not found in git repo".to_string(),
        });
    }

    let manifest_content =
        fs::read_to_string(&manifest_path).map_err(|source| SnpmError::ReadFile {
            path: manifest_path.clone(),
            source,
        })?;

    let manifest: Manifest =
        serde_json::from_str(&manifest_content).map_err(|source| SnpmError::ParseJson {
            path: manifest_path.clone(),
            source,
        })?;

    let version = manifest
        .version
        .clone()
        .unwrap_or_else(|| "0.0.0".to_string());
    let dist_url = format!("file://{}", repo_dir.display());
    let registry_version = super::super::registry_version_from_manifest(manifest, &dist_url);

    let mut versions = BTreeMap::new();
    versions.insert(version.clone(), registry_version);

    let mut dist_tags = BTreeMap::new();
    dist_tags.insert("latest".to_string(), version);

    Ok(RegistryPackage {
        versions,
        time: BTreeMap::new(),
        dist_tags,
    })
}
