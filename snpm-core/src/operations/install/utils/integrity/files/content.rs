use super::super::super::types::IntegrityState;

pub(super) fn integrity_content(state: &IntegrityState) -> String {
    format!(
        "lockfile: {}\npatches: {}\n",
        state.lockfile_hash, state.patch_hash
    )
}

pub(super) fn legacy_integrity_content(state: &IntegrityState) -> String {
    format!("lockfile: {}\n", state.lockfile_hash)
}
