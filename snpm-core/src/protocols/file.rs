use crate::project::Manifest;
use crate::registry::RegistryPackage;
use crate::{Result, SnpmConfig, SnpmError};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub async fn fetch_package(_config: &SnpmConfig, path_str: &str) -> Result<RegistryPackage> {
    let cwd = env::current_dir().map_err(|e| SnpmError::Io {
        path: PathBuf::from("."),
        source: e,
    })?;

    let path = Path::new(path_str);
    let abs_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    };

    if !abs_path.exists() {
        return Err(SnpmError::ResolutionFailed {
            name: path_str.to_string(),
            range: "latest".to_string(),
            reason: format!("File path does not exist: {}", abs_path.display()),
        });
    }

    if abs_path.is_dir() {
        let manifest_path = abs_path.join("package.json");
        let manifest_content =
            fs::read_to_string(&manifest_path).map_err(|e| SnpmError::ReadFile {
                path: manifest_path.clone(),
                source: e,
            })?;

        let manifest: Manifest =
            serde_json::from_str(&manifest_content).map_err(|e| SnpmError::ParseJson {
                path: manifest_path.clone(),
                source: e,
            })?;

        let version = manifest
            .version
            .clone()
            .unwrap_or_else(|| "0.0.0".to_string());

        let reg_version = super::registry_version_from_manifest(
            manifest,
            &format!("file://{}", abs_path.display()),
        );

        let mut versions = BTreeMap::new();
        versions.insert(version.clone(), reg_version);

        let mut dist_tags = BTreeMap::new();
        dist_tags.insert("latest".to_string(), version);

        Ok(RegistryPackage {
            versions,
            time: BTreeMap::new(),
            dist_tags,
        })
    } else {
        Err(SnpmError::ResolutionFailed {
            name: path_str.to_string(),
            range: "latest".to_string(),
            reason: "Single file dependencies are not yet fully supported (expected directory)"
                .to_string(),
        })
    }
}
