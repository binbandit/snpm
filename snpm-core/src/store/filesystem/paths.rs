use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn sanitize_name(name: &str) -> String {
    name.replace('/', "_")
}

pub fn package_root_dir(pkg_dir: &Path) -> PathBuf {
    let candidate = pkg_dir.join("package");
    if candidate.is_dir() {
        return candidate;
    }

    if let Ok(entries) = fs::read_dir(pkg_dir) {
        let mut dirs = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir()
                && path
                    .file_name()
                    .is_some_and(|name| name != ".snpm_complete")
            {
                dirs.push(path);
            }
        }

        if dirs.len() == 1 && dirs[0].join("package.json").is_file() {
            return dirs[0].clone();
        }
    }

    pkg_dir.to_path_buf()
}
