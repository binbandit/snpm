use crate::{Project, Result, SnpmConfig, SnpmError, console};

use std::fs;

use super::package_name;
use super::symlinks::{create_symlink, replace_path};

/// Link the current package globally (run from the package directory).
/// Creates a symlink in the global directory pointing to the current package.
pub fn link_global(config: &SnpmConfig, project: &Project) -> Result<()> {
    let name = package_name(project, "link")?;
    let global_dir = config.global_dir();

    fs::create_dir_all(&global_dir).map_err(|source| SnpmError::WriteFile {
        path: global_dir.clone(),
        source,
    })?;

    let link_target = global_dir.join(name);
    crate::linker::fs::ensure_parent_dir(&link_target)?;
    replace_path(&link_target);
    create_symlink(&project.root, &link_target)?;

    let global_bin_dir = config.global_bin_dir();
    fs::create_dir_all(&global_bin_dir).map_err(|source| SnpmError::WriteFile {
        path: global_bin_dir.clone(),
        source,
    })?;
    crate::linker::bins::link_bins(&link_target, &global_bin_dir, name).ok();

    console::info(&format!("Linked {} -> {}", name, project.root.display()));
    Ok(())
}

/// Remove a global link for the current package.
pub fn unlink_global(config: &SnpmConfig, project: &Project) -> Result<()> {
    let name = package_name(project, "unlink")?;
    let link_path = config.global_dir().join(name);

    if link_path.symlink_metadata().is_ok() {
        replace_path(&link_path);
        console::info(&format!("Unlinked {}", name));
    } else {
        console::info(&format!("{} is not linked", name));
    }

    Ok(())
}
