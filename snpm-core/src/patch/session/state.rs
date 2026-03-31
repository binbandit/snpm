use crate::{Result, SnpmError};

use std::fs;
use std::path::Path;

use super::super::types::PatchSession;

pub fn read_patch_session(patch_dir: &Path) -> Result<PatchSession> {
    let session_path = patch_dir.join(super::super::SESSION_MARKER);

    if !session_path.exists() {
        return Err(SnpmError::PatchSessionNotFound {
            path: patch_dir.to_path_buf(),
        });
    }

    let content = fs::read_to_string(&session_path).map_err(|source| SnpmError::ReadFile {
        path: session_path.clone(),
        source,
    })?;

    serde_json::from_str(&content).map_err(|source| SnpmError::ParseJson {
        path: session_path,
        source,
    })
}

pub fn cleanup_patch_session(patch_dir: &Path) -> Result<()> {
    if patch_dir.exists() {
        fs::remove_dir_all(patch_dir).map_err(|source| SnpmError::Io {
            path: patch_dir.to_path_buf(),
            source,
        })?;
    }

    Ok(())
}
