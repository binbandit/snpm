use crate::{Result, SnpmError};

use std::fs;
use std::path::{Path, PathBuf};

pub fn symlink_is_correct(link: &Path, expected_target: &Path) -> bool {
    match fs::read_link(link) {
        Ok(current_target) => current_target == expected_target && expected_target.exists(),
        Err(_) => false,
    }
}

/// Strip the package name from a virtual store package location to get
/// the containing `node_modules` directory. Handles scoped packages
/// (`@scope/name` → 2 levels) and unscoped packages (`name` → 1 level).
pub fn package_node_modules(location: &Path, name: &str) -> Option<PathBuf> {
    let depth = if name.contains('/') { 2 } else { 1 };
    let mut current = location;

    for _ in 0..depth {
        current = current.parent()?;
    }

    Some(current.to_path_buf())
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

#[cfg(test)]
mod tests {
    use super::{ensure_parent_dir, package_node_modules, symlink_is_correct};

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
    fn symlink_is_correct_false_for_broken_link() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("missing-target");
        let link = dir.path().join("link");

        #[cfg(unix)]
        std::os::unix::fs::symlink(&target, &link).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(&target, &link).unwrap();

        assert!(!symlink_is_correct(&link, &target));
    }

    #[test]
    fn ensure_parent_dir_creates_parents() {
        let dir = tempfile::tempdir().unwrap();
        let deep_path = dir.path().join("a/b/c/file.txt");
        ensure_parent_dir(&deep_path).unwrap();
        assert!(dir.path().join("a/b/c").is_dir());
    }
}
