use crate::copying::clone_or_copy_file;
use crate::store::read_package_filesystem_shape_lossy;
use crate::{Result, SnpmError};

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

pub fn copy_dir(source: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest).map_err(|source_err| SnpmError::WriteFile {
        path: dest.to_path_buf(),
        source: source_err,
    })?;

    if let Some(shape) = read_package_filesystem_shape_lossy(source) {
        for directory in &shape.directories {
            let destination = dest.join(directory);
            fs::create_dir_all(&destination).map_err(|source| SnpmError::WriteFile {
                path: destination,
                source,
            })?;
        }

        for file in &shape.files {
            let from = source.join(file);
            let to = dest.join(file);
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent).map_err(|source| SnpmError::WriteFile {
                    path: parent.to_path_buf(),
                    source,
                })?;
            }
            clone_or_copy_file(&from, &to)
                .map_err(|source| SnpmError::WriteFile { path: to, source })?;
        }

        return Ok(());
    }

    for entry in fs::read_dir(source).map_err(|source_err| SnpmError::ReadFile {
        path: source.to_path_buf(),
        source: source_err,
    })? {
        let entry = entry.map_err(|source_err| SnpmError::ReadFile {
            path: source.to_path_buf(),
            source: source_err,
        })?;
        let file_type = entry
            .file_type()
            .map_err(|source_err| SnpmError::ReadFile {
                path: entry.path(),
                source: source_err,
            })?;

        let from = entry.path();
        let to = dest.join(entry.file_name());

        if file_type.is_symlink() {
            return Err(SnpmError::Io {
                path: from,
                source: std::io::Error::new(
                    ErrorKind::InvalidData,
                    "refusing to copy symlink from package store",
                ),
            });
        }

        if file_type.is_dir() {
            copy_dir(&from, &to)?;
        } else {
            clone_or_copy_file(&from, &to).map_err(|source_err| SnpmError::WriteFile {
                path: to,
                source: source_err,
            })?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::copy_dir;
    use crate::store::PACKAGE_METADATA_FILE;

    #[test]
    fn copy_dir_copies_files() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        let dst = dir.path().join("dst");

        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("file.txt"), "hello").unwrap();

        copy_dir(&src, &dst).unwrap();

        assert!(dst.join("file.txt").is_file());
        assert_eq!(
            std::fs::read_to_string(dst.join("file.txt")).unwrap(),
            "hello"
        );
    }

    #[test]
    fn copy_dir_copies_nested() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        let dst = dir.path().join("dst");

        std::fs::create_dir_all(src.join("sub")).unwrap();
        std::fs::write(src.join("sub/nested.txt"), "nested").unwrap();

        copy_dir(&src, &dst).unwrap();

        assert_eq!(
            std::fs::read_to_string(dst.join("sub/nested.txt")).unwrap(),
            "nested"
        );
    }

    #[test]
    fn copy_dir_rejects_symlinks() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        let dst = dir.path().join("dst");

        std::fs::create_dir_all(&src).unwrap();
        let target = dir.path().join("outside");
        std::fs::write(&target, "data").unwrap();

        #[cfg(unix)]
        std::os::unix::fs::symlink(&target, src.join("link")).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_file(&target, src.join("link")).unwrap();

        let result = copy_dir(&src, &dst);
        assert!(result.is_err());
    }

    #[cfg(unix)]
    #[test]
    fn copy_dir_uses_indexed_shape_without_scanning_source_tree() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        let dst = dir.path().join("dst");

        std::fs::create_dir_all(src.join("sub")).unwrap();
        std::fs::write(src.join("sub/nested.txt"), "nested").unwrap();
        std::fs::write(
            src.join(PACKAGE_METADATA_FILE),
            r#"{
                "filesystem": {
                    "directories": ["sub"],
                    "files": ["sub/nested.txt", ".snpm-package-metadata.json"]
                }
            }"#,
        )
        .unwrap();
        std::os::unix::fs::symlink(dir.path().join("outside"), src.join("bad-link")).unwrap();

        copy_dir(&src, &dst).unwrap();

        assert_eq!(
            std::fs::read_to_string(dst.join("sub/nested.txt")).unwrap(),
            "nested"
        );
        assert!(dst.join(PACKAGE_METADATA_FILE).is_file());
    }
}
