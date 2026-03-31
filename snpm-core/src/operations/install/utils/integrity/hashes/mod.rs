mod lockfile;
mod patches;
mod state;

pub use lockfile::{NO_PATCH_HASH, compute_lockfile_hash};
pub use patches::{compute_project_patch_hash, compute_workspace_patch_hash};
pub use state::{build_project_integrity_state, build_workspace_integrity_state};

#[cfg(test)]
mod tests;
