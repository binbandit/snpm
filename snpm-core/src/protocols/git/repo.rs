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
    // Repo and committish come from dependency specs (possibly a
    // transitive dependency's lockfile). A value starting with `-`
    // would be parsed by git as an option — `--upload-pack=<cmd>`
    // executes arbitrary commands during resolve, before any lifecycle
    // script policy applies. Refuse them outright.
    reject_option_like(&spec.repo, raw)?;
    if let Some(committish) = spec.committish.as_deref() {
        reject_option_like(committish, raw)?;
    }

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

fn reject_option_like(value: &str, raw: &str) -> Result<()> {
    if value.starts_with('-') {
        return Err(SnpmError::ResolutionFailed {
            name: raw.to_string(),
            range: value.to_string(),
            reason: "git repository/committish must not start with '-'".to_string(),
        });
    }
    Ok(())
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
            [
                "clone", "--depth", "1", "--branch", revision, "--", repo, "repo",
            ],
            "clone",
        )
        .await
        .is_ok()
    {
        return Ok(());
    }

    run_git_command(
        cache_dir,
        raw,
        "latest",
        ["clone", "--", repo, "repo"],
        "clone",
    )
    .await
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

/// Git-level flags that disable the `ext::` and `fd::` remote-helper
/// transports, which can execute arbitrary commands during a
/// clone/fetch. Even though the spec router should never route such a URL
/// to the git resolver, this is defense-in-depth: legitimate transports
/// (https/ssh/git/file) are unaffected.
const GIT_TRANSPORT_HARDENING: [&str; 4] = [
    "-c",
    "protocol.ext.allow=never",
    "-c",
    "protocol.fd.allow=never",
];

async fn run_git_command<const N: usize>(
    dir: &Path,
    raw: &str,
    range: &str,
    args: [&str; N],
    action: &str,
) -> Result<()> {
    let output = Command::new("git")
        .current_dir(dir)
        .args(GIT_TRANSPORT_HARDENING)
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
