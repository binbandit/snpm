use crate::console;
use crate::resolve::types::ResolvedPackage;
use crate::{Result, SnpmConfig, SnpmError};

use std::path::Path;
use std::time::Instant;

use super::fetch::download_and_extract;
use super::filesystem::reset_package_dir;
use super::limits::extraction_semaphore;

pub(super) async fn materialize_remote_package(
    config: &SnpmConfig,
    package: &ResolvedPackage,
    client: &reqwest::Client,
    package_dir: &Path,
) -> Result<()> {
    let _extract_permit =
        extraction_semaphore()
            .acquire()
            .await
            .map_err(|error| SnpmError::Internal {
                reason: format!(
                    "extraction semaphore closed while unpacking {}@{}: {error}",
                    package.id.name, package.id.version
                ),
            })?;

    {
        let target_dir = package_dir.to_path_buf();
        tokio::task::spawn_blocking(move || reset_package_dir(&target_dir))
            .await
            .map_err(|error| SnpmError::StoreTask {
                reason: error.to_string(),
            })??;
    }

    let materialize_started = Instant::now();
    let tarball = download_and_extract(
        config,
        &package.id.name,
        &package.tarball,
        package.integrity.as_deref(),
        client,
        package_dir,
    )
    .await?;

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
