use crate::{Result, SnpmError};

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

pub fn copy_dir(source: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest).map_err(|source_err| SnpmError::WriteFile {
        path: dest.to_path_buf(),
        source: source_err,
    })?;

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
            fs::copy(&from, &to).map_err(|source_err| SnpmError::WriteFile {
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
}
