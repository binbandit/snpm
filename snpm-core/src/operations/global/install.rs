use crate::linker::bins::link_bins_flat;
use crate::{Result, SnpmConfig, SnpmError};

use std::fs;

use super::super::install;
use super::super::install::manifest::parse_requested_spec;
use super::super::install::utils::{FrozenLockfileMode, InstallOptions};
use super::project::global_project;
use super::shell::print_path_setup_hint;

pub async fn install_global(config: &SnpmConfig, packages: Vec<String>) -> Result<()> {
    if packages.is_empty() {
        return Ok(());
    }

    let global_bin_dir = config.global_bin_dir();
    fs::create_dir_all(&global_bin_dir).map_err(|source| SnpmError::WriteFile {
        path: global_bin_dir.clone(),
        source,
    })?;

    // Global installs run through the standard project pipeline against
    // the managed global project, so the whole dependency tree is
    // resolved, stored, and linked — not just the top-level package.
    let mut project = global_project(config)?;
    install::install(
        config,
        &mut project,
        InstallOptions {
            requested: packages.clone(),
            dev: false,
            include_dev: true,
            frozen_lockfile: FrozenLockfileMode::Prefer,
            strict_no_lockfile: false,
            force: false,
            silent_summary: false,
        },
    )
    .await?;

    // Expose each requested package's bins flat in the global bin dir —
    // the directory users put on PATH.
    for spec in &packages {
        let name = parse_requested_spec(spec).name;
        let package_dir = project.root.join("node_modules").join(&name);
        link_bins_flat(&package_dir, &global_bin_dir, &name)?;
    }

    println!();
    print_path_setup_hint(&global_bin_dir);

    Ok(())
}
