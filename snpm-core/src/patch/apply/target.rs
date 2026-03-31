use crate::linker::fs::copy_dir;
use crate::{Result, SnpmError};

use std::fs;
use std::path::Path;

pub fn materialize_patch_target(target_dir: &Path, store_path: &Path) -> Result<()> {
    if let Ok(metadata) = target_dir.symlink_metadata() {
        if metadata.is_dir() && !metadata.file_type().is_symlink() {
            fs::remove_dir_all(target_dir).map_err(|source| SnpmError::Io {
                path: target_dir.to_path_buf(),
                source,
            })?;
        } else {
            fs::remove_file(target_dir).map_err(|source| SnpmError::Io {
                path: target_dir.to_path_buf(),
                source,
            })?;
        }
    }

    crate::linker::fs::ensure_parent_dir(target_dir)?;
    copy_dir(store_path, target_dir)
}
