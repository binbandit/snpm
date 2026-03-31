use crate::{Project, Result, SnpmConfig, SnpmError, console};

use std::fs;

use super::symlinks::{create_symlink, replace_path};

/// Link a globally-linked package into the current project's node_modules.
pub fn link_local(project: &Project, config: &SnpmConfig, package_name: &str) -> Result<()> {
    let source = config.global_dir().join(package_name);

    if !source.exists() {
        return Err(SnpmError::ManifestInvalid {
            path: source,
            reason: format!(
                "{} is not linked globally. Run 'snpm link' in the package directory first.",
                package_name
            ),
        });
    }

    let node_modules = project.root.join("node_modules");
    fs::create_dir_all(&node_modules).map_err(|source| SnpmError::WriteFile {
        path: node_modules.clone(),
        source,
    })?;

    let destination = node_modules.join(package_name);
    crate::linker::fs::ensure_parent_dir(&destination)?;
    replace_path(&destination);
    create_symlink(&source, &destination)?;

    let bin_dir = node_modules.join(".bin");
    fs::create_dir_all(&bin_dir).ok();
    crate::linker::bins::link_bins(&destination, &node_modules, package_name).ok();

    console::info(&format!(
        "Linked {} into {}",
        package_name,
        project.root.display()
    ));
    Ok(())
}

/// Remove a linked package from node_modules.
pub fn unlink_local(project: &Project, package_name: &str) -> Result<()> {
    let destination = project.root.join("node_modules").join(package_name);

    if destination.symlink_metadata().is_ok() {
        replace_path(&destination);
        console::info(&format!(
            "Unlinked {} from {}",
            package_name,
            project.root.display()
        ));
    } else {
        console::info(&format!("{} is not linked in this project", package_name));
    }

    Ok(())
}
