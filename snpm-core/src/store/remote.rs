use crate::console;
use crate::resolve::types::ResolvedPackage;
use crate::{Result, SnpmConfig, SnpmError};

use std::path::Path;
use std::time::Instant;

use super::fetch::download_and_extract;
use super::filesystem::atomic_finalize_extracted_dir;

pub(super) async fn materialize_remote_package(
    config: &SnpmConfig,
    package: &ResolvedPackage,
    client: &reqwest::Client,
    package_dir: &Path,
) -> Result<()> {
    // Note: the extraction semaphore is acquired inside `download_and_extract`
    // around the actual CPU-bound decompress/extract work. Holding it here
    // would also cover the network-bound stream, capping concurrency at the
    // (much smaller) extract budget and serializing parallel downloads.

    let parent_dir = package_dir.parent().ok_or_else(|| SnpmError::Internal {
        reason: format!("package directory has no parent: {}", package_dir.display()),
    })?;

    let staged_dir = {
        let parent_for_blocking = parent_dir.to_path_buf();
        tokio::task::spawn_blocking(move || create_staging_dir(&parent_for_blocking))
            .await
            .map_err(|error| SnpmError::StoreTask {
                reason: error.to_string(),
            })??
    };

    let materialize_started = Instant::now();
    let tarball_result = download_and_extract(
        config,
        &package.id.name,
        &package.tarball,
        package.integrity.as_deref(),
        client,
        &staged_dir,
    )
    .await;

    let tarball = match tarball_result {
        Ok(tarball) => tarball,
        Err(error) => {
            let staged_for_cleanup = staged_dir.clone();
            let _ = tokio::task::spawn_blocking(move || {
                let _ = std::fs::remove_dir_all(&staged_for_cleanup);
            })
            .await;
            return Err(error);
        }
    };

    let final_dir = package_dir.to_path_buf();
    let staged_for_finalize = staged_dir.clone();
    tokio::task::spawn_blocking(move || {
        atomic_finalize_extracted_dir(&staged_for_finalize, &final_dir)
    })
    .await
    .map_err(|error| SnpmError::StoreTask {
        reason: error.to_string(),
    })??;

    match tarball.source() {
        super::fetch::TarballSource::Downloaded => console::verbose(&format!(
            "streamed and extracted tarball for {}@{} ({} bytes) in {:.3}s",
            package.id.name,
            package.id.version,
            tarball.size_bytes(),
            materialize_started.elapsed().as_secs_f64()
        )),
        super::fetch::TarballSource::BlobCache => console::verbose(&format!(
            "reused verified tarball blob for {}@{} ({}) in {:.3}s",
            package.id.name,
            package.id.version,
            tarball.path().display(),
            materialize_started.elapsed().as_secs_f64()
        )),
    }

    Ok(())
}

fn create_staging_dir(parent: &Path) -> Result<std::path::PathBuf> {
    std::fs::create_dir_all(parent).map_err(|source| SnpmError::WriteFile {
        path: parent.to_path_buf(),
        source,
    })?;
    let staging = tempfile::Builder::new()
        .prefix(".snpm-extract-")
        .tempdir_in(parent)
        .map_err(|source| SnpmError::WriteFile {
            path: parent.to_path_buf(),
            source,
        })?;
    Ok(staging.keep())
}
