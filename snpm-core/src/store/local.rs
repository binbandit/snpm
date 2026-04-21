use crate::console;
use crate::resolve::types::ResolvedPackage;
use crate::{Result, SnpmError};

use std::path::{Path, PathBuf};

use super::archive::unpack_tarball_file;
use super::filesystem::{copy_dir_all, reset_package_dir};
use super::integrity::verify_integrity_file;
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

    let _extract_permit =
        extraction_semaphore()
            .acquire()
            .await
            .map_err(|error| SnpmError::Internal {
                reason: format!(
                    "extraction semaphore closed while installing local package: {error}"
                ),
            })?;
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
        verify_integrity_file(&package.tarball, package.integrity.as_deref(), &source_path)?;
        unpack_tarball_file(package_dir, &source_path)?;
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
