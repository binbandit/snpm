use std::path::Path;

use super::resolve_relative_path;

pub(in crate::resolve) fn normalize_dependency_range(tarball: &str, dep_range: &str) -> String {
    if let Some(package_path) = tarball.strip_prefix("file://")
        && let Some(relative_path) = dep_range.strip_prefix("file:")
    {
        return format!(
            "file:{}",
            resolve_relative_path(Path::new(package_path), relative_path).display()
        );
    }

    dep_range.to_string()
}
