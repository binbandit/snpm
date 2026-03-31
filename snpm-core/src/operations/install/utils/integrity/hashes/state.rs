use super::super::super::types::IntegrityState;
use super::lockfile::compute_lockfile_hash;
use super::patches::{compute_project_patch_hash, compute_workspace_patch_hash};
use crate::resolve::ResolutionGraph;
use crate::{Project, Result, Workspace};

pub fn build_project_integrity_state(
    project: &Project,
    graph: &ResolutionGraph,
) -> Result<IntegrityState> {
    Ok(IntegrityState {
        lockfile_hash: compute_lockfile_hash(graph),
        patch_hash: compute_project_patch_hash(project)?,
    })
}

pub fn build_workspace_integrity_state(
    workspace: &Workspace,
    graph: &ResolutionGraph,
) -> Result<IntegrityState> {
    Ok(IntegrityState {
        lockfile_hash: compute_lockfile_hash(graph),
        patch_hash: compute_workspace_patch_hash(workspace)?,
    })
}
