use crate::lockfile;
use crate::operations::why::index::{ReverseIndex, build_reverse_index};
use crate::operations::why::pattern::matches_pattern;
use crate::resolve::PackageId;
use crate::{Project, Result, SnpmError, Workspace};

use std::path::PathBuf;

pub(super) fn load_why_context(
    project: &Project,
    patterns: &[String],
) -> Result<(ReverseIndex, Vec<PackageId>)> {
    let lockfile_path = find_lockfile_path(project)?;
    let lock = lockfile::read(&lockfile_path)?;
    let graph = lockfile::to_graph(&lock);
    let index = build_reverse_index(&graph);

    let mut targets: Vec<PackageId> = graph
        .packages
        .keys()
        .filter(|id| {
            patterns
                .iter()
                .any(|pattern| matches_pattern(&id.name, pattern))
        })
        .cloned()
        .collect();
    targets.sort();

    Ok((index, targets))
}

fn find_lockfile_path(project: &Project) -> Result<PathBuf> {
    let workspace = Workspace::discover(&project.root)?;
    let lockfile_path = workspace
        .as_ref()
        .map(|workspace| workspace.root.join("snpm-lock.yaml"))
        .unwrap_or_else(|| project.root.join("snpm-lock.yaml"));

    if lockfile_path.is_file() {
        Ok(lockfile_path)
    } else {
        Err(SnpmError::Lockfile {
            path: lockfile_path,
            reason: "snpm-lock.yaml is missing. Run `snpm install` first.".into(),
        })
    }
}
