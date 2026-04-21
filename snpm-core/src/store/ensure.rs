use crate::config::OfflineMode;
use crate::console;
use crate::resolve::types::ResolvedPackage;
use crate::{Result, SnpmConfig, SnpmError};

use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use super::filesystem::{package_root_dir, sanitize_name};
use super::local::materialize_local_package;
use super::remote::materialize_remote_package;
use super::{PACKAGE_METADATA_FILE, persist_package_metadata};

/// Ensure a package is in the store (Online mode).
pub async fn ensure_package(
    config: &SnpmConfig,
    package: &ResolvedPackage,
    client: &reqwest::Client,
) -> Result<PathBuf> {
    ensure_package_with_offline(config, package, client, OfflineMode::Online).await
}

/// Ensure a package is in the store, respecting offline mode.
pub async fn ensure_package_with_offline(
    config: &SnpmConfig,
    package: &ResolvedPackage,
    client: &reqwest::Client,
    offline_mode: OfflineMode,
) -> Result<PathBuf> {
    let start = Instant::now();
    let package_dir = package_dir(config, package);
    let marker = package_dir.join(".snpm_complete");

    if marker.is_file() {
        let root = package_root_dir(&package_dir);
        if let Err(error) = backfill_package_metadata(&package_dir, &root) {
            console::verbose(&format!(
                "failed to backfill package metadata for {}@{}: {}",
                package.id.name, package.id.version, error
            ));
        }
        console::verbose(&format!(
            "store hit: {}@{} ({})",
            package.id.name,
            package.id.version,
            root.display()
        ));
        return Ok(root);
    }

    if matches!(offline_mode, OfflineMode::Offline) {
        return Err(SnpmError::OfflineRequired {
            resource: format!("package {}@{}", package.id.name, package.id.version),
        });
    }

    console::verbose(&format!(
        "store miss: {}@{}; downloading from {}",
        package.id.name, package.id.version, package.tarball
    ));

    if package.tarball.starts_with("file://") {
        materialize_local_package(package, &package_dir).await?;
    } else {
        materialize_remote_package(config, package, client, &package_dir).await?;
    }

    fs::write(&marker, []).map_err(|source| SnpmError::WriteFile {
        path: marker.clone(),
        source,
    })?;

    let root = package_root_dir(&package_dir);
    if let Err(error) = persist_package_metadata(&package_dir, &root) {
        console::verbose(&format!(
            "failed to write package metadata for {}@{}: {}",
            package.id.name, package.id.version, error
        ));
    }
    console::verbose(&format!(
        "ensure_package complete for {}@{} in {:.3}s (root={})",
        package.id.name,
        package.id.version,
        start.elapsed().as_secs_f64(),
        root.display()
    ));

    Ok(root)
}

fn package_dir(config: &SnpmConfig, package: &ResolvedPackage) -> PathBuf {
    config
        .packages_dir()
        .join(sanitize_name(&package.id.name))
        .join(&package.id.version)
}

fn backfill_package_metadata(
    package_dir: &std::path::Path,
    package_root: &std::path::Path,
) -> Result<()> {
    let root_metadata = package_root.join(PACKAGE_METADATA_FILE);
    let store_metadata = package_dir.join(PACKAGE_METADATA_FILE);
    let store_metadata_present = package_dir == package_root || store_metadata.is_file();

    if root_metadata.is_file() && store_metadata_present {
        return Ok(());
    }

    persist_package_metadata(package_dir, package_root)
}
