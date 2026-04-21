use crate::copying::clone_or_copy_file;
use crate::{Result, SnpmError};

use std::fs;
use std::path::{Path, PathBuf};

use super::super::types::PatchSession;

pub fn prepare_patch_directory(
    package_name: &str,
    package_version: &str,
    source_path: &Path,
) -> Result<PathBuf> {
    let safe_name = package_name.replace('/', "+");
    let dir_name = format!("{}@{}", safe_name, package_version);
    let patch_dir = std::env::temp_dir()
        .join(format!("snpm-patch-{}", std::process::id()))
        .join(&dir_name);

    if patch_dir.exists() {
        fs::remove_dir_all(&patch_dir).map_err(|source| SnpmError::Io {
            path: patch_dir.clone(),
            source,
        })?;
    }

    copy_directory(source_path, &patch_dir)?;

    let session = PatchSession {
        package_name: package_name.to_string(),
        package_version: package_version.to_string(),
        original_path: source_path.to_path_buf(),
    };

    let session_path = patch_dir.join(super::super::SESSION_MARKER);
    let session_json =
        serde_json::to_string_pretty(&session).map_err(|error| SnpmError::SerializeJson {
            path: session_path.clone(),
            reason: error.to_string(),
        })?;

    fs::write(&session_path, session_json).map_err(|source| SnpmError::WriteFile {
        path: session_path,
        source,
    })?;

    Ok(patch_dir)
}

fn copy_directory(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst).map_err(|source| SnpmError::WriteFile {
        path: dst.to_path_buf(),
        source,
    })?;

    for entry in fs::read_dir(src)
        .map_err(|source| SnpmError::ReadFile {
            path: src.to_path_buf(),
            source,
        })?
        .flatten()
    {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if name_str == "node_modules" || name_str == ".git" {
            continue;
        }

        let src_path = entry.path();
        let dst_path = dst.join(&name);

        if src_path.is_dir() {
            copy_directory(&src_path, &dst_path)?;
        } else {
            clone_or_copy_file(&src_path, &dst_path).map_err(|source| SnpmError::Io {
                path: dst_path,
                source,
            })?;
        }
    }

    Ok(())
}
