use crate::linker::bins::link_bins_flat;
use crate::linker::fs::remove_symlink;
use crate::{Result, SnpmConfig};

use std::fs;
use std::path::Path;

use super::super::install;
use super::super::install::manifest::parse_requested_spec;
use super::super::install::utils::FrozenLockfileMode;
use super::project::{global_project, migrate_legacy_global_packages};

pub async fn remove_global(config: &SnpmConfig, packages: Vec<String>) -> Result<()> {
    if packages.is_empty() {
        return Ok(());
    }

    // Route through the standard remove path against the managed global
    // project: the manifest entries are dropped and the reinstall prunes
    // node_modules and the lockfile. Migrating first makes packages from
    // the pre-project layout removable through the same path.
    let mut project = global_project(config)?;
    migrate_legacy_global_packages(&mut project)?;
    install::remove(
        config,
        &mut project,
        packages.clone(),
        FrozenLockfileMode::Prefer,
        false,
    )
    .await?;

    let global_bin_dir = config.global_bin_dir();

    // Drop the removed packages' launchers by ownership: a launcher whose
    // target lives under the removed package's root link must go even if
    // the target still resolves — hoisting can keep node_modules/<name>
    // alive as another global package's transitive dep, and such a
    // launcher would otherwise keep a removed command on PATH forever.
    let global_node_modules = project.root.join("node_modules");
    for spec in &packages {
        let name = parse_requested_spec(spec).name;
        prune_bins_owned_by(&global_bin_dir, &global_node_modules.join(&name));
    }

    // The removed packages' remaining root links are gone now, so their
    // launchers dangle — prune every dangling symlink rather than
    // guessing bin names.
    prune_dangling_bins(&global_bin_dir);

    // Surviving packages keep their launchers healthy (a migrated legacy
    // package's launchers point at its deleted pre-project copy until
    // they are re-linked here).
    for name in project.manifest.dependencies.keys() {
        let package_dir = global_node_modules.join(name);
        link_bins_flat(&package_dir, &global_bin_dir, name).ok();
    }

    Ok(())
}

/// Remove launchers whose symlink target lives under `package_root`.
fn prune_bins_owned_by(bin_dir: &Path, package_root: &Path) {
    let Ok(entries) = fs::read_dir(bin_dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_symlink() {
            continue;
        }

        if let Ok(target) = fs::read_link(&path)
            && target.starts_with(package_root)
        {
            remove_symlink(&path);
        }
    }
}

pub(super) fn prune_dangling_bins(bin_dir: &Path) {
    let Ok(entries) = fs::read_dir(bin_dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_symlink() && fs::metadata(&path).is_err() {
            remove_symlink(&path);
        }
    }
}
