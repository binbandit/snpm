use crate::{Result, SnpmError};

use flate2::read::GzDecoder;
use std::fs;
#[cfg(test)]
use std::io::Cursor;
use std::io::{ErrorKind, Read};
use std::path::Path;
use tar::Archive;

use super::paths::safe_join;

#[cfg(test)]
pub(crate) fn unpack_tarball(pkg_dir: &Path, data: Vec<u8>) -> Result<()> {
    unpack_tarball_reader(pkg_dir, Cursor::new(data))
}

pub(crate) fn unpack_tarball_file(pkg_dir: &Path, tarball_path: &Path) -> Result<()> {
    let file = fs::File::open(tarball_path).map_err(|source| SnpmError::ReadFile {
        path: tarball_path.to_path_buf(),
        source,
    })?;

    unpack_tarball_reader(pkg_dir, file)
}

fn unpack_tarball_reader<R: Read>(pkg_dir: &Path, reader: R) -> Result<()> {
    let decoder = GzDecoder::new(reader);
    let mut archive = Archive::new(decoder);

    let entries = archive.entries().map_err(|source| SnpmError::Archive {
        path: pkg_dir.to_path_buf(),
        source,
    })?;

    for entry in entries {
        let mut entry = entry.map_err(|source| SnpmError::Archive {
            path: pkg_dir.to_path_buf(),
            source,
        })?;

        let rel_path = entry.path().map_err(|source| SnpmError::Archive {
            path: pkg_dir.to_path_buf(),
            source,
        })?;

        let Some(dest_path) = safe_join(pkg_dir, &rel_path) else {
            return invalid_entry_error(
                pkg_dir,
                format!(
                    "archive entry escapes extraction root: {}",
                    rel_path.display()
                ),
            );
        };

        let entry_type = entry.header().entry_type();
        if entry_type.is_symlink() || entry_type.is_hard_link() {
            return invalid_entry_error(
                pkg_dir,
                format!(
                    "archive contains forbidden symlink/hardlink entry: {}",
                    rel_path.display()
                ),
            );
        }

        if entry_type.is_dir() {
            fs::create_dir_all(&dest_path).map_err(|source| SnpmError::WriteFile {
                path: dest_path.clone(),
                source,
            })?;
            continue;
        }

        if !entry_type.is_file() {
            continue;
        }

        crate::linker::fs::ensure_parent_dir(&dest_path)?;
        entry
            .unpack(&dest_path)
            .map_err(|source| SnpmError::Archive {
                path: dest_path,
                source,
            })?;
    }

    Ok(())
}

fn invalid_entry_error<T>(pkg_dir: &Path, message: String) -> Result<T> {
    Err(SnpmError::Archive {
        path: pkg_dir.to_path_buf(),
        source: std::io::Error::new(ErrorKind::InvalidData, message),
    })
}
