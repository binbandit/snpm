use crate::{Result, SnpmError};

use std::path::Path;
use std::process::{Command, Stdio};

pub fn apply_patch(target_dir: &Path, patch_path: &Path) -> Result<()> {
    if !patch_path.exists() {
        return Err(SnpmError::PatchNotFound {
            name: patch_path.to_string_lossy().to_string(),
            reason: "patch file does not exist".to_string(),
        });
    }

    let current_dir = target_dir.canonicalize().map_err(|source| SnpmError::Io {
        path: target_dir.to_path_buf(),
        source,
    })?;

    let output = Command::new("patch")
        .args(["-p1", "-i"])
        .arg(patch_path)
        .current_dir(&current_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|_| SnpmError::PatchApply {
            name: patch_path.to_string_lossy().to_string(),
            reason: "patch command not found - please install patch".to_string(),
        })?;

    if output.status.success() {
        Ok(())
    } else {
        Err(SnpmError::PatchApply {
            name: patch_path.to_string_lossy().to_string(),
            reason: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
}
