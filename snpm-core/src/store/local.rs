use crate::console;
use crate::resolve::types::ResolvedPackage;
use crate::{Result, SnpmError};

use std::fs;
use std::path::{Path, PathBuf};

use super::archive::unpack_tarball;
use super::filesystem::{copy_dir_all, reset_package_dir};
use super::integrity::verify_integrity;
use super::limits::extraction_semaphore;

pub(super) async fn materialize_local_package(
    package: &ResolvedPackage,
    package_dir: &Path,
) -> Result<()> {
    let source_path = local_tarball_path(package)?;

    console::verbose(&format!(
        "installing local package from {}",
        source_path.display()
    ));

    let _extract_permit = extraction_semaphore().acquire().await.unwrap();
    let prep_dir = package_dir.to_path_buf();

    tokio::task::spawn_blocking(move || -> Result<()> { reset_package_dir(&prep_dir) })
        .await
        .map_err(|error| SnpmError::StoreTask {
            reason: error.to_string(),
        })??;

    if source_path.is_dir() {
        let destination = package_dir.join("package");
        copy_dir_all(&source_path, &destination).map_err(|source| SnpmError::Io {
            path: destination.clone(),
            source,
        })?;
    } else {
        let bytes = fs::read(&source_path).map_err(|source| SnpmError::ReadFile {
            path: source_path.clone(),
            source,
        })?;
        verify_integrity(&package.tarball, package.integrity.as_deref(), &bytes)?;
        unpack_tarball(package_dir, bytes)?;
    }

    Ok(())
}

fn local_tarball_path(package: &ResolvedPackage) -> Result<PathBuf> {
    let Some(path) = package.tarball.strip_prefix("file://") else {
        return Err(SnpmError::Internal {
            reason: "expected file:// tarball for local package materialization".into(),
        });
    };

    Ok(PathBuf::from(path))
}
