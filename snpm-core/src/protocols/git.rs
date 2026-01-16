use crate::project::Manifest;
use crate::registry::RegistryPackage;
use crate::{Result, SnpmConfig, SnpmError};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use tokio::process::Command;

pub async fn fetch_package(config: &SnpmConfig, url: &str) -> Result<RegistryPackage> {
    let git_spec = parse_git_spec(url)?;
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
            .arg("--tags")
            .arg("--prune")
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

        checkout_repo(&repo_dir, git_spec.committish.as_deref(), url).await?;
    } else {
        let status = Command::new("git")
            .current_dir(&cache_dir)
            .arg("clone")
            .arg(&git_spec.repo)
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

        checkout_repo(&repo_dir, git_spec.committish.as_deref(), url).await?;
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

struct GitSpec {
    repo: String,
    committish: Option<String>,
}

fn parse_git_spec(raw: &str) -> Result<GitSpec> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(SnpmError::ResolutionFailed {
            name: raw.to_string(),
            range: "latest".to_string(),
            reason: "git spec is empty".to_string(),
        });
    }

    let (repo_part, committish) = split_committish(trimmed);
    let repo = normalize_git_repo(repo_part).map_err(|reason| SnpmError::ResolutionFailed {
        name: raw.to_string(),
        range: "latest".to_string(),
        reason,
    })?;

    Ok(GitSpec { repo, committish })
}

fn split_committish(value: &str) -> (&str, Option<String>) {
    if let Some(idx) = value.rfind('#') {
        let committish = value[idx + 1..].trim();
        let base = value[..idx].trim();
        let committish = if committish.is_empty() {
            None
        } else {
            Some(committish.to_string())
        };
        (base, committish)
    } else {
        (value, None)
    }
}

fn normalize_git_repo(raw: &str) -> std::result::Result<String, String> {
    let mut repo = raw.trim().to_string();
    if repo.is_empty() {
        return Err("git repo URL is empty".to_string());
    }

    if let Some(stripped) = repo.strip_prefix("git+") {
        repo = stripped.to_string();
    }

    if let Some(stripped) = repo.strip_prefix("git:") {
        repo = normalize_git_colon(stripped)?;
    }

    Ok(correct_ssh_url(&repo))
}

fn normalize_git_colon(rest: &str) -> std::result::Result<String, String> {
    let cleaned = rest.trim();
    if cleaned.is_empty() {
        return Err("git: URL is missing a repo".to_string());
    }

    if cleaned.starts_with("//") {
        return Ok(format!("git:{cleaned}"));
    }

    if cleaned.contains("://") {
        return Ok(cleaned.to_string());
    }

    if cleaned.contains('@') {
        return Ok(cleaned.to_string());
    }

    if let Some((host, path)) = cleaned.split_once(':') {
        if host.is_empty() || path.is_empty() {
            return Err(format!("git: URL is invalid: {cleaned}"));
        }
        return Ok(format!("ssh://git@{host}/{path}"));
    }

    Ok(format!("git://{cleaned}"))
}

fn correct_ssh_url(url: &str) -> String {
    let Some(rest) = url.strip_prefix("ssh://") else {
        return url.to_string();
    };

    let mut parts = rest.splitn(2, '/');
    let auth_host = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("");

    let Some(idx) = auth_host.rfind(':') else {
        return url.to_string();
    };

    let (left, right) = auth_host.split_at(idx);
    let candidate = right.trim_start_matches(':');

    if candidate.is_empty() || candidate.chars().all(|c| c.is_ascii_digit()) {
        return url.to_string();
    }

    let mut corrected = String::from("ssh://");
    corrected.push_str(left);
    corrected.push('/');
    corrected.push_str(candidate);
    if !path.is_empty() {
        corrected.push('/');
        corrected.push_str(path);
    }

    corrected
}

async fn checkout_repo(repo_dir: &Path, committish: Option<&str>, raw: &str) -> Result<()> {
    let target = if let Some(rev) = committish {
        rev.to_string()
    } else {
        default_remote_head(repo_dir)
            .await
            .unwrap_or_else(|| "HEAD".to_string())
    };

    let status = Command::new("git")
        .current_dir(repo_dir)
        .arg("checkout")
        .arg("--force")
        .arg(&target)
        .status()
        .await
        .map_err(|e| SnpmError::ResolutionFailed {
            name: raw.to_string(),
            range: target.clone(),
            reason: format!("Failed to run git checkout: {}", e),
        })?;

    if !status.success() {
        return Err(SnpmError::ResolutionFailed {
            name: raw.to_string(),
            range: target,
            reason: "git checkout failed".to_string(),
        });
    }

    Ok(())
}

async fn default_remote_head(repo_dir: &Path) -> Option<String> {
    let output = Command::new("git")
        .current_dir(repo_dir)
        .arg("symbolic-ref")
        .arg("--quiet")
        .arg("--short")
        .arg("refs/remotes/origin/HEAD")
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let head = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if head.is_empty() {
        None
    } else {
        Some(head)
    }
}
