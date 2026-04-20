mod pnpm;

use super::types::Lockfile;
use crate::{Result, SnpmConfig};

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompatibleLockfileKind {
    Pnpm,
}

impl CompatibleLockfileKind {
    pub fn filename(self) -> &'static str {
        match self {
            CompatibleLockfileKind::Pnpm => "pnpm-lock.yaml",
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
    let path = project_root.join(CompatibleLockfileKind::Pnpm.filename());
    path.is_file().then_some(CompatibleLockfile {
        kind: CompatibleLockfileKind::Pnpm,
        path,
    })
}

pub fn read_compatible_lockfile(
    source: &CompatibleLockfile,
    config: &SnpmConfig,
) -> Result<Lockfile> {
    match source.kind {
        CompatibleLockfileKind::Pnpm => pnpm::read(&source.path, config),
    }
}
