use std::path::Path;

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

/// Delete a symlink regardless of platform: on Windows a directory
/// symlink is removed with remove_dir, not remove_file — using only the
/// latter silently leaves stale links in place.
pub fn remove_symlink(path: &Path) {
    if std::fs::remove_file(path).is_err() {
        std::fs::remove_dir(path).ok();
    }
}
