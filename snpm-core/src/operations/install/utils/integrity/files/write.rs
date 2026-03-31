use super::super::super::types::IntegrityState;
use crate::{Project, Result, SnpmError};

use std::fs;
use std::path::Path;

use super::content::integrity_content;

pub fn write_integrity_file(project: &Project, state: &IntegrityState) -> Result<()> {
    write_integrity_path(&project.root.join("node_modules"), state)
}

pub fn write_integrity_path(node_modules: &Path, state: &IntegrityState) -> Result<()> {
    if !node_modules.is_dir() {
        return Ok(());
    }

    let integrity_path = node_modules.join(".snpm-integrity");
    fs::write(&integrity_path, integrity_content(state)).map_err(|source| SnpmError::WriteFile {
        path: integrity_path,
        source,
    })
}
