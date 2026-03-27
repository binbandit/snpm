use crate::{LinkBackend, Result, SnpmConfig, SnpmError};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static RESOLVED_AUTO_BACKEND: OnceLock<LinkBackend> = OnceLock::new();

pub fn symlink_is_correct(link: &Path, expected_target: &Path) -> bool {
    match fs::read_link(link) {
        Ok(current_target) => current_target == expected_target,
        Err(_) => false,
    }
}

/// Strip the package name from a virtual store package location to get
/// the containing `node_modules` directory. Handles scoped packages
/// (`@scope/name` → 2 levels) and unscoped packages (`name` → 1 level).
pub fn package_node_modules(location: &Path, name: &str) -> Option<PathBuf> {
    let depth = if name.contains('/') { 2 } else { 1 };
    let mut p = location;
    for _ in 0..depth {
        p = p.parent()?;
    }
    Some(p.to_path_buf())
}

pub fn ensure_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| SnpmError::WriteFile {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    Ok(())
}

pub fn link_dir(config: &SnpmConfig, source: &Path, dest: &Path) -> Result<()> {
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
                    "refusing to link symlink from package store",
                ),
            });
        }

        if file_type.is_dir() {
            link_dir(config, &from, &to)?;
        } else {
            link_file(config, &from, &to)?;
        }
    }

    Ok(())
}

/// Detect the best link strategy by probing once, then cache the result.
fn resolve_auto_backend(from: &Path, to: &Path) -> LinkBackend {
    *RESOLVED_AUTO_BACKEND.get_or_init(|| {
        if reflink_copy::reflink(from, to).is_ok() {
            let _ = fs::remove_file(to);
            return LinkBackend::Reflink;
        }

        if fs::hard_link(from, to).is_ok() {
            let _ = fs::remove_file(to);
            return LinkBackend::Hardlink;
        }

        if symlink_file_entry(from, to).is_ok() {
            let _ = fs::remove_file(to);
            return LinkBackend::Symlink;
        }

        LinkBackend::Copy
    })
}

fn link_file(config: &SnpmConfig, from: &Path, to: &Path) -> Result<()> {
    let backend = match config.link_backend {
        LinkBackend::Auto => resolve_auto_backend(from, to),
        other => other,
    };

    match backend {
        LinkBackend::Auto => unreachable!(),
        LinkBackend::Reflink => {
            if reflink_copy::reflink(from, to).is_err() {
                fs::copy(from, to).map_err(|source_err| SnpmError::WriteFile {
                    path: to.to_path_buf(),
                    source: source_err,
                })?;
            }
        }
        LinkBackend::Hardlink => {
            if fs::hard_link(from, to).is_err() {
                fs::copy(from, to).map_err(|source_err| SnpmError::WriteFile {
                    path: to.to_path_buf(),
                    source: source_err,
                })?;
            }
        }
        LinkBackend::Symlink => {
            if symlink_file_entry(from, to).is_err() {
                fs::copy(from, to).map_err(|source_err| SnpmError::WriteFile {
                    path: to.to_path_buf(),
                    source: source_err,
                })?;
            }
        }
        LinkBackend::Copy => {
            fs::copy(from, to).map_err(|source_err| SnpmError::WriteFile {
                path: to.to_path_buf(),
                source: source_err,
            })?;
        }
    }

    Ok(())
}

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

#[cfg(unix)]
pub fn symlink_dir_entry(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::symlink;
    symlink(from, to)
}

#[cfg(windows)]
pub fn symlink_dir_entry(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::os::windows::fs::symlink_dir;
    symlink_dir(from, to)
}

#[cfg(unix)]
pub fn symlink_file_entry(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::symlink;
    symlink(from, to)
}

#[cfg(windows)]
pub fn symlink_file_entry(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::os::windows::fs::symlink_file;
    symlink_file(from, to)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn package_node_modules_unscoped() {
        let location =
            PathBuf::from("/project/node_modules/.snpm/lodash@4.17.21/node_modules/lodash");
        let result = package_node_modules(&location, "lodash");
        assert_eq!(
            result,
            Some(PathBuf::from(
                "/project/node_modules/.snpm/lodash@4.17.21/node_modules"
            ))
        );
    }

    #[test]
    fn package_node_modules_scoped() {
        let location = PathBuf::from(
            "/project/node_modules/.snpm/@types+node@18.0.0/node_modules/@types/node",
        );
        let result = package_node_modules(&location, "@types/node");
        assert_eq!(
            result,
            Some(PathBuf::from(
                "/project/node_modules/.snpm/@types+node@18.0.0/node_modules"
            ))
        );
    }

    #[test]
    fn symlink_is_correct_returns_false_for_nonexistent() {
        let result = symlink_is_correct(
            &PathBuf::from("/nonexistent/link"),
            &PathBuf::from("/nonexistent/target"),
        );
        assert!(!result);
    }

    #[test]
    fn symlink_is_correct_with_actual_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target");
        std::fs::create_dir_all(&target).unwrap();

        let link = dir.path().join("link");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&target, &link).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(&target, &link).unwrap();

        assert!(symlink_is_correct(&link, &target));
    }

    #[test]
    fn symlink_is_correct_wrong_target() {
        let dir = tempfile::tempdir().unwrap();
        let target1 = dir.path().join("target1");
        let target2 = dir.path().join("target2");
        std::fs::create_dir_all(&target1).unwrap();
        std::fs::create_dir_all(&target2).unwrap();

        let link = dir.path().join("link");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&target1, &link).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(&target1, &link).unwrap();

        assert!(!symlink_is_correct(&link, &target2));
    }

    #[test]
    fn ensure_parent_dir_creates_parents() {
        let dir = tempfile::tempdir().unwrap();
        let deep_path = dir.path().join("a/b/c/file.txt");
        ensure_parent_dir(&deep_path).unwrap();
        assert!(dir.path().join("a/b/c").is_dir());
    }

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
