use super::spec::GitSpec;
use crate::{Result, SnpmConfig, SnpmError};
use std::fs;
use std::path::{Path, PathBuf};
use tokio::process::Command;

pub(super) fn repo_cache_dir(config: &SnpmConfig, url: &str) -> PathBuf {
    let safe_name = url.replace(|c: char| !c.is_alphanumeric(), "_");
    config.cache_dir.join("git").join(safe_name)
}

pub(super) async fn prepare_repo(cache_dir: &Path, spec: &GitSpec, raw: &str) -> Result<PathBuf> {
    ensure_cache_dir(cache_dir)?;

    let repo_dir = cache_dir.join("repo");
    if repo_dir.exists() {
        fetch_repo(&repo_dir, spec.committish.as_deref(), raw).await?;
    } else {
        clone_repo(cache_dir, &spec.repo, spec.committish.as_deref(), raw).await?;
    }

    checkout_repo(&repo_dir, spec.committish.as_deref(), raw).await?;
    Ok(repo_dir)
}

fn ensure_cache_dir(cache_dir: &Path) -> Result<()> {
    if cache_dir.exists() {
        return Ok(());
    }

    fs::create_dir_all(cache_dir).map_err(|source| SnpmError::Io {
        path: cache_dir.to_path_buf(),
        source,
    })
}

async fn fetch_repo(repo_dir: &Path, committish: Option<&str>, raw: &str) -> Result<()> {
    if let Some(revision) = committish
        && run_git_command(
            repo_dir,
            raw,
            revision,
            ["fetch", "--depth", "1", "origin", revision],
            "fetch",
        )
        .await
        .is_ok()
    {
        return Ok(());
    }

    run_git_command(
        repo_dir,
        raw,
        "latest",
        ["fetch", "--all", "--tags", "--prune"],
        "fetch",
    )
    .await
}

async fn clone_repo(
    cache_dir: &Path,
    repo: &str,
    committish: Option<&str>,
    raw: &str,
) -> Result<()> {
    if let Some(revision) = committish
        && run_git_command(
            cache_dir,
            raw,
            revision,
            ["clone", "--depth", "1", "--branch", revision, repo, "repo"],
            "clone",
        )
        .await
        .is_ok()
    {
        return Ok(());
    }

    run_git_command(cache_dir, raw, "latest", ["clone", repo, "repo"], "clone").await
}

async fn checkout_repo(repo_dir: &Path, committish: Option<&str>, raw: &str) -> Result<()> {
    let target = if let Some(revision) = committish {
        revision.to_string()
    } else {
        default_remote_head(repo_dir)
            .await
            .unwrap_or_else(|| "HEAD".to_string())
    };

    run_git_command(
        repo_dir,
        raw,
        &target,
        ["checkout", "--force", target.as_str()],
        "checkout",
    )
    .await
}

async fn run_git_command<const N: usize>(
    dir: &Path,
    raw: &str,
    range: &str,
    args: [&str; N],
    action: &str,
) -> Result<()> {
    let output = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .await
        .map_err(|error| SnpmError::ResolutionFailed {
            name: raw.to_string(),
            range: range.to_string(),
            reason: format!("Failed to run git {action}: {error}"),
        })?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(SnpmError::ResolutionFailed {
        name: raw.to_string(),
        range: range.to_string(),
        reason: format!("git {action} failed: {}", stderr.trim()),
    })
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
    if head.is_empty() { None } else { Some(head) }
}
