use crate::console;
use crate::resolve::types::ResolvedPackage;
use crate::{Result, SnpmError};

use std::path::{Path, PathBuf};

use super::archive::unpack_tarball_file;
use super::filesystem::{atomic_finalize_extracted_dir, copy_dir_all};
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

    let parent_dir = package_dir.parent().ok_or_else(|| SnpmError::Internal {
        reason: format!(
            "package directory has no parent: {}",
            package_dir.display()
        ),
    })?;
    let parent_for_blocking = parent_dir.to_path_buf();
    let staged_dir = tokio::task::spawn_blocking(move || stage_dir(&parent_for_blocking))
        .await
        .map_err(|error| SnpmError::StoreTask {
            reason: error.to_string(),
        })??;

    let result: Result<()> = (|| {
        if source_path.is_dir() {
            let destination = staged_dir.join("package");
            copy_dir_all(&source_path, &destination).map_err(|source| SnpmError::Io {
                path: destination.clone(),
                source,
            })?;
        } else {
            verify_integrity_file(&package.tarball, package.integrity.as_deref(), &source_path)?;
            unpack_tarball_file(&staged_dir, &source_path)?;
        }
        Ok(())
    })();

    if let Err(error) = result {
        let _ = std::fs::remove_dir_all(&staged_dir);
        return Err(error);
    }

    let final_dir = package_dir.to_path_buf();
    let staged_for_finalize = staged_dir.clone();
    tokio::task::spawn_blocking(move || {
        atomic_finalize_extracted_dir(&staged_for_finalize, &final_dir)
    })
    .await
    .map_err(|error| SnpmError::StoreTask {
        reason: error.to_string(),
    })??;

    Ok(())
}

fn stage_dir(parent: &Path) -> Result<PathBuf> {
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

fn local_tarball_path(package: &ResolvedPackage) -> Result<PathBuf> {
    let Some(path) = package.tarball.strip_prefix("file://") else {
        return Err(SnpmError::Internal {
            reason: "expected file:// tarball for local package materialization".into(),
        });
    };

    Ok(PathBuf::from(path))
}
