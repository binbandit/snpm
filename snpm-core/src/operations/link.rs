use crate::{Project, Result, SnpmConfig, SnpmError, console};
use std::fs;
use std::path::Path;

/// Link the current package globally (run from the package directory).
/// Creates a symlink in the global directory pointing to the current package.
pub fn link_global(config: &SnpmConfig, project: &Project) -> Result<()> {
    let name = project
        .manifest
        .name
        .as_deref()
        .ok_or_else(|| SnpmError::ManifestInvalid {
            path: project.manifest_path.clone(),
            reason: "package.json must have a \"name\" field to link".into(),
        })?;

    let global_dir = config.global_dir();
    fs::create_dir_all(&global_dir).map_err(|source| SnpmError::WriteFile {
        path: global_dir.clone(),
        source,
    })?;

    let link_target = global_dir.join(name);

    crate::linker::fs::ensure_parent_dir(&link_target)?;

    // Remove existing link/dir
    if link_target.symlink_metadata().is_ok() {
        if link_target.is_dir()
            && !link_target
                .symlink_metadata()
                .is_ok_and(|m| m.file_type().is_symlink())
        {
            fs::remove_dir_all(&link_target).ok();
        } else {
            fs::remove_file(&link_target).ok();
        }
    }

    create_symlink(&project.root, &link_target)?;

    // Link bins to global bin dir
    let global_bin_dir = config.global_bin_dir();
    fs::create_dir_all(&global_bin_dir).map_err(|source| SnpmError::WriteFile {
        path: global_bin_dir.clone(),
        source,
    })?;
    crate::linker::bins::link_bins(&link_target, &global_bin_dir, name).ok();

    console::info(&format!("Linked {} -> {}", name, project.root.display()));
    Ok(())
}

/// Link a globally-linked package into the current project's node_modules.
pub fn link_local(project: &Project, config: &SnpmConfig, package_name: &str) -> Result<()> {
    let global_dir = config.global_dir();
    let source = global_dir.join(package_name);

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

    let dest = node_modules.join(package_name);

    // Create parent for scoped packages
    crate::linker::fs::ensure_parent_dir(&dest)?;

    // Remove existing
    if dest.symlink_metadata().is_ok() {
        if dest.is_dir()
            && !dest
                .symlink_metadata()
                .is_ok_and(|m| m.file_type().is_symlink())
        {
            fs::remove_dir_all(&dest).ok();
        } else {
            fs::remove_file(&dest).ok();
        }
    }

    create_symlink(&source, &dest)?;

    // Link bins
    let bin_dir = node_modules.join(".bin");
    fs::create_dir_all(&bin_dir).ok();
    crate::linker::bins::link_bins(&dest, &node_modules, package_name).ok();

    console::info(&format!(
        "Linked {} into {}",
        package_name,
        project.root.display()
    ));
    Ok(())
}

/// Remove a global link for the current package.
pub fn unlink_global(config: &SnpmConfig, project: &Project) -> Result<()> {
    let name = project
        .manifest
        .name
        .as_deref()
        .ok_or_else(|| SnpmError::ManifestInvalid {
            path: project.manifest_path.clone(),
            reason: "package.json must have a \"name\" field to unlink".into(),
        })?;

    let global_dir = config.global_dir();
    let link_path = global_dir.join(name);

    if link_path.symlink_metadata().is_ok() {
        if link_path.is_dir()
            && !link_path
                .symlink_metadata()
                .is_ok_and(|m| m.file_type().is_symlink())
        {
            fs::remove_dir_all(&link_path).ok();
        } else {
            fs::remove_file(&link_path).ok();
        }
        console::info(&format!("Unlinked {}", name));
    } else {
        console::info(&format!("{} is not linked", name));
    }

    Ok(())
}

/// Remove a linked package from node_modules.
pub fn unlink_local(project: &Project, package_name: &str) -> Result<()> {
    let dest = project.root.join("node_modules").join(package_name);

    if dest.symlink_metadata().is_ok() {
        if dest.is_dir()
            && !dest
                .symlink_metadata()
                .is_ok_and(|m| m.file_type().is_symlink())
        {
            fs::remove_dir_all(&dest).ok();
        } else {
            fs::remove_file(&dest).ok();
        }
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

fn create_symlink(source: &Path, dest: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(source, dest).map_err(|source_err| SnpmError::WriteFile {
            path: dest.to_path_buf(),
            source: source_err,
        })?;
    }

    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_dir(source, dest).map_err(|source_err| {
            SnpmError::WriteFile {
                path: dest.to_path_buf(),
                source: source_err,
            }
        })?;
    }

    Ok(())
}
