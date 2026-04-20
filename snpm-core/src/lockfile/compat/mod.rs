mod bun;
mod npm;
mod pnpm;
mod yarn;

use super::types::Lockfile;
use crate::{Result, SnpmConfig};

use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompatibleLockfileKind {
    Pnpm,
    Bun,
    Yarn,
    NpmShrinkwrap,
    Npm,
}

impl CompatibleLockfileKind {
    pub fn filename(self) -> &'static str {
        match self {
            CompatibleLockfileKind::Pnpm => "pnpm-lock.yaml",
            CompatibleLockfileKind::Bun => "bun.lock",
            CompatibleLockfileKind::Yarn => "yarn.lock",
            CompatibleLockfileKind::NpmShrinkwrap => "npm-shrinkwrap.json",
            CompatibleLockfileKind::Npm => "package-lock.json",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompatibleLockfile {
    pub kind: CompatibleLockfileKind,
    pub path: PathBuf,
}

impl CompatibleLockfile {
    pub fn label(&self) -> &'static str {
        self.kind.filename()
    }
}

pub fn detect_compatible_lockfile(project_root: &Path) -> Option<CompatibleLockfile> {
    compatible_lockfile_candidates(project_root)
        .into_iter()
        .find(|candidate| candidate.path.is_file())
}

pub fn read_compatible_lockfile(
    source: &CompatibleLockfile,
    config: &SnpmConfig,
) -> Result<Lockfile> {
    match source.kind {
        CompatibleLockfileKind::Pnpm => pnpm::read(&source.path, config),
        CompatibleLockfileKind::Bun => bun::read(&source.path, config),
        CompatibleLockfileKind::Yarn => yarn::read(&source.path, config),
        CompatibleLockfileKind::NpmShrinkwrap | CompatibleLockfileKind::Npm => {
            npm::read(&source.path, config)
        }
    }
}

fn compatible_lockfile_candidates(project_root: &Path) -> Vec<CompatibleLockfile> {
    let mut candidates = Vec::new();

    if let Some(branch) = current_git_branch(project_root) {
        candidates.push(CompatibleLockfile {
            kind: CompatibleLockfileKind::Pnpm,
            path: project_root.join(pnpm_branch_lockfile_name(&branch)),
        });
    }

    candidates.push(CompatibleLockfile {
        kind: CompatibleLockfileKind::Pnpm,
        path: project_root.join(CompatibleLockfileKind::Pnpm.filename()),
    });
    candidates.push(CompatibleLockfile {
        kind: CompatibleLockfileKind::Bun,
        path: project_root.join(CompatibleLockfileKind::Bun.filename()),
    });
    candidates.push(CompatibleLockfile {
        kind: CompatibleLockfileKind::Yarn,
        path: project_root.join(CompatibleLockfileKind::Yarn.filename()),
    });
    candidates.push(CompatibleLockfile {
        kind: CompatibleLockfileKind::NpmShrinkwrap,
        path: project_root.join(CompatibleLockfileKind::NpmShrinkwrap.filename()),
    });
    candidates.push(CompatibleLockfile {
        kind: CompatibleLockfileKind::Npm,
        path: project_root.join(CompatibleLockfileKind::Npm.filename()),
    });

    candidates
}

fn current_git_branch(project_root: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["-C"])
        .arg(project_root)
        .args(["branch", "--show-current"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let branch = String::from_utf8(output.stdout).ok()?.trim().to_string();
    if branch.is_empty() {
        None
    } else {
        Some(branch)
    }
}

fn pnpm_branch_lockfile_name(branch: &str) -> String {
    format!("pnpm-lock.{}.yaml", branch.replace('/', "!"))
}

#[cfg(test)]
mod tests {
    use super::{CompatibleLockfileKind, detect_compatible_lockfile, pnpm_branch_lockfile_name};

    #[test]
    fn pnpm_branch_lockfile_name_replaces_slashes() {
        assert_eq!(
            pnpm_branch_lockfile_name("feature/lockfile-import"),
            "pnpm-lock.feature!lockfile-import.yaml"
        );
    }

    #[test]
    fn detect_compatible_lockfile_falls_back_to_base_name() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(CompatibleLockfileKind::Pnpm.filename());
        std::fs::write(&path, "lockfileVersion: '9.0'\nimporters: {}\n").unwrap();

        let detected = detect_compatible_lockfile(dir.path()).unwrap();
        assert_eq!(detected.path, path);
    }

    #[test]
    fn detect_compatible_lockfile_prefers_npm_shrinkwrap() {
        let dir = tempfile::tempdir().unwrap();
        let shrinkwrap = dir
            .path()
            .join(CompatibleLockfileKind::NpmShrinkwrap.filename());
        let package_lock = dir.path().join(CompatibleLockfileKind::Npm.filename());

        std::fs::write(
            &package_lock,
            "{ \"lockfileVersion\": 3, \"packages\": { \"\": {} } }",
        )
        .unwrap();
        std::fs::write(
            &shrinkwrap,
            "{ \"lockfileVersion\": 3, \"packages\": { \"\": {} } }",
        )
        .unwrap();

        let detected = detect_compatible_lockfile(dir.path()).unwrap();
        assert_eq!(detected.kind, CompatibleLockfileKind::NpmShrinkwrap);
        assert_eq!(detected.path, shrinkwrap);
    }

    #[test]
    fn detect_compatible_lockfile_prefers_bun_before_npm() {
        let dir = tempfile::tempdir().unwrap();
        let bun = dir.path().join(CompatibleLockfileKind::Bun.filename());
        let package_lock = dir.path().join(CompatibleLockfileKind::Npm.filename());

        std::fs::write(
            &bun,
            "{\n  \"lockfileVersion\": 1,\n  \"workspaces\": { \"\": {} },\n  \"packages\": {}\n}\n",
        )
        .unwrap();
        std::fs::write(
            &package_lock,
            "{ \"lockfileVersion\": 3, \"packages\": { \"\": {} } }",
        )
        .unwrap();

        let detected = detect_compatible_lockfile(dir.path()).unwrap();
        assert_eq!(detected.kind, CompatibleLockfileKind::Bun);
        assert_eq!(detected.path, bun);
    }

    #[test]
    fn detect_compatible_lockfile_prefers_yarn_before_npm() {
        let dir = tempfile::tempdir().unwrap();
        let yarn = dir.path().join(CompatibleLockfileKind::Yarn.filename());
        let package_lock = dir.path().join(CompatibleLockfileKind::Npm.filename());

        std::fs::write(&yarn, "# yarn lockfile v1\n").unwrap();
        std::fs::write(
            &package_lock,
            "{ \"lockfileVersion\": 3, \"packages\": { \"\": {} } }",
        )
        .unwrap();

        let detected = detect_compatible_lockfile(dir.path()).unwrap();
        assert_eq!(detected.kind, CompatibleLockfileKind::Yarn);
        assert_eq!(detected.path, yarn);
    }
}
