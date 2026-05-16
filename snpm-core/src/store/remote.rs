use crate::console;
use crate::resolve::types::ResolvedPackage;
use crate::{Result, SnpmConfig};

use std::path::Path;
use std::time::Instant;

use super::fetch::download_and_extract;

pub(super) async fn materialize_remote_package(
    config: &SnpmConfig,
    package: &ResolvedPackage,
    client: &reqwest::Client,
    package_dir: &Path,
) -> Result<()> {
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
