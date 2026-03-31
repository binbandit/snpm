use crate::console;
use crate::resolve::types::ResolvedPackage;
use crate::{Result, SnpmConfig, SnpmError};

use std::path::Path;
use std::time::Instant;

use super::archive::unpack_tarball;
use super::fetch::download_and_verify_tarball;
use super::filesystem::reset_package_dir;
use super::limits::{download_semaphore, extraction_semaphore};

pub(super) async fn materialize_remote_package(
    config: &SnpmConfig,
    package: &ResolvedPackage,
    client: &reqwest::Client,
    package_dir: &Path,
) -> Result<()> {
    let download_started = Instant::now();
    let bytes = {
        let _download_permit = download_semaphore().acquire().await.unwrap();
        let bytes = download_and_verify_tarball(
            config,
            &package.tarball,
            package.integrity.as_deref(),
            client,
        )
        .await?;
        drop(_download_permit);
        bytes
    };

    console::verbose(&format!(
        "downloaded and verified tarball for {}@{} ({} bytes) in {:.3}s",
        package.id.name,
        package.id.version,
        bytes.len(),
        download_started.elapsed().as_secs_f64()
    ));

    let _extract_permit = extraction_semaphore().acquire().await.unwrap();
    let unpack_started = Instant::now();
    let target_dir = package_dir.to_path_buf();

    tokio::task::spawn_blocking(move || -> Result<()> {
        reset_package_dir(&target_dir)?;
        unpack_tarball(&target_dir, bytes)
    })
    .await
    .map_err(|error| SnpmError::StoreTask {
        reason: error.to_string(),
    })??;

    console::verbose(&format!(
        "unpacked tarball for {}@{} in {:.3}s",
        package.id.name,
        package.id.version,
        unpack_started.elapsed().as_secs_f64()
    ));

    Ok(())
}
