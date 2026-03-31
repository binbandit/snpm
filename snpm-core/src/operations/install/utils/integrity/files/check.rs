use super::super::super::types::IntegrityState;
use super::super::hashes::NO_PATCH_HASH;
use crate::Project;

use std::fs;
use std::path::Path;

use super::content::{integrity_content, legacy_integrity_content};

pub fn check_integrity_file(project: &Project, state: &IntegrityState) -> bool {
    check_integrity_path(&project.root.join("node_modules"), state)
}

pub fn check_integrity_path(node_modules: &Path, state: &IntegrityState) -> bool {
    let integrity_path = node_modules.join(".snpm-integrity");

    match fs::read_to_string(&integrity_path) {
        Ok(content) => {
            content == integrity_content(state)
                || (state.patch_hash == NO_PATCH_HASH && content == legacy_integrity_content(state))
        }
        Err(_) => false,
    }
}
