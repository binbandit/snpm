use crate::project::Manifest;
use crate::registry::RegistryPackage;
use crate::{Result, SnpmConfig, SnpmError};
use std::collections::BTreeMap;
use std::fs;
use tokio::process::Command;

pub async fn fetch_package(config: &SnpmConfig, url: &str) -> Result<RegistryPackage> {
    let safe_name = url.replace(|c: char| !c.is_alphanumeric(), "_");
    let cache_dir = config.cache_dir.join("git").join(&safe_name);

    if !cache_dir.exists() {
        fs::create_dir_all(&cache_dir).map_err(|e| SnpmError::Io {
            path: cache_dir.clone(),
            source: e,
        })?;
    }

    let repo_dir = cache_dir.join("repo");

    if repo_dir.exists() {
        let status = Command::new("git")
            .current_dir(&repo_dir)
            .arg("fetch")
            .arg("--all")
            .status()
            .await
            .map_err(|e| SnpmError::ResolutionFailed {
                name: url.to_string(),
                range: "latest".to_string(),
                reason: format!("Failed to run git fetch: {}", e),
            })?;

        if !status.success() {
            return Err(SnpmError::ResolutionFailed {
                name: url.to_string(),
                range: "latest".to_string(),
                reason: "git fetch failed".to_string(),
            });
        }
    } else {
        let mut clean_url = url.to_string();
        let mut committish = None;

        if let Some(idx) = url.rfind('#') {
            committish = Some(&url[idx + 1..]);
            clean_url = url[..idx].to_string();
        }

        let status = Command::new("git")
            .current_dir(&cache_dir)
            .arg("clone")
            .arg(&clean_url)
            .arg("repo")
            .status()
            .await
            .map_err(|e| SnpmError::ResolutionFailed {
                name: url.to_string(),
                range: "latest".to_string(),
                reason: format!("Failed to run git clone: {}", e),
            })?;

        if !status.success() {
            return Err(SnpmError::ResolutionFailed {
                name: url.to_string(),
                range: "latest".to_string(),
                reason: "git clone failed".to_string(),
            });
        }

        if let Some(rev) = committish {
            let status = Command::new("git")
                .current_dir(&repo_dir)
                .arg("checkout")
                .arg(rev)
                .status()
                .await
                .map_err(|e| SnpmError::ResolutionFailed {
                    name: url.to_string(),
                    range: rev.to_string(),
                    reason: format!("Failed to run git checkout: {}", e),
                })?;

            if !status.success() {
                return Err(SnpmError::ResolutionFailed {
                    name: url.to_string(),
                    range: rev.to_string(),
                    reason: "git checkout failed".to_string(),
                });
            }
        }
    }

    let manifest_path = repo_dir.join("package.json");
    if !manifest_path.exists() {
        return Err(SnpmError::ResolutionFailed {
            name: url.to_string(),
            range: "latest".to_string(),
            reason: "package.json not found in git repo".to_string(),
        });
    }

    let manifest_content = fs::read_to_string(&manifest_path).map_err(|e| SnpmError::ReadFile {
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

    let reg_version =
        super::registry_version_from_manifest(manifest, &format!("file://{}", repo_dir.display()));

    let mut versions = BTreeMap::new();
    versions.insert(version.clone(), reg_version);

    let mut dist_tags = BTreeMap::new();
    dist_tags.insert("latest".to_string(), version);

    Ok(RegistryPackage {
        versions,
        time: BTreeMap::new(),
        dist_tags,
    })
}
