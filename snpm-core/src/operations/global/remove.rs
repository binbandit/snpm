use crate::{Result, SnpmConfig};

use std::fs;
use std::path::Path;

use super::super::install;
use super::super::install::utils::FrozenLockfileMode;
use super::project::global_project;

pub async fn remove_global(config: &SnpmConfig, packages: Vec<String>) -> Result<()> {
    if packages.is_empty() {
        return Ok(());
    }

    // Route through the standard remove path against the managed global
    // project: the manifest entries are dropped and the reinstall prunes
    // node_modules and the lockfile.
    let mut project = global_project(config)?;
    install::remove(
        config,
        &mut project,
        packages,
        FrozenLockfileMode::Prefer,
        false,
    )
    .await?;

    // The removed packages' root links are gone now, so their global
    // launchers dangle — prune every dangling symlink rather than
    // guessing bin names.
    prune_dangling_bins(&config.global_bin_dir());

    Ok(())
}

fn prune_dangling_bins(bin_dir: &Path) {
    let Ok(entries) = fs::read_dir(bin_dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_symlink() && fs::metadata(&path).is_err() {
            fs::remove_file(&path).ok();
        }
    }
}
