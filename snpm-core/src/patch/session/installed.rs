use crate::{Project, Result, SnpmError};

use std::fs;
use std::path::PathBuf;

pub fn find_installed_package(project: &Project, name: &str) -> Result<(String, PathBuf)> {
    let package_path = project.root.join("node_modules").join(name);
    let package_json_path = package_path.join("package.json");

    if !package_json_path.exists() {
        return Err(SnpmError::PackageNotInstalled {
            name: name.to_string(),
            version: "unknown".to_string(),
        });
    }

    let content = fs::read_to_string(&package_json_path).map_err(|source| SnpmError::ReadFile {
        path: package_json_path.clone(),
        source,
    })?;

    let json: serde_json::Value =
        serde_json::from_str(&content).map_err(|source| SnpmError::ParseJson {
            path: package_json_path.clone(),
            source,
        })?;

    let version = json
        .get("version")
        .and_then(|value| value.as_str())
        .ok_or_else(|| SnpmError::ManifestInvalid {
            path: package_json_path,
            reason: "missing version field".to_string(),
        })?
        .to_string();

    Ok((version, package_path))
}
