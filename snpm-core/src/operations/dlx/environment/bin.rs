use crate::console;
use crate::{Result, SnpmError};

use std::path::{Path, PathBuf};

pub(super) fn resolve_bin_path(temp_path: &Path, package_name: &str) -> Result<PathBuf> {
    let bin_name = package_name.rsplit('/').next().unwrap_or(package_name);
    let bin_dir = temp_path.join("node_modules").join(".bin");
    let bin_path = bin_dir.join(bin_name);

    if bin_path.exists() {
        return Ok(bin_path);
    }

    if let Ok(entries) = std::fs::read_dir(&bin_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() || path.is_symlink() {
                console::verbose(&format!(
                    "Default binary {} not found, utilizing {}",
                    bin_name,
                    path.display()
                ));
                return Ok(path);
            }
        }
    }

    Err(SnpmError::ScriptRun {
        name: package_name.to_string(),
        reason: "Binary not found".to_string(),
    })
}
