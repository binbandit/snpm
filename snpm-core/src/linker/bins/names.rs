use std::path::{Component, Path, PathBuf};

pub(in crate::linker::bins) fn sanitize_bin_name(name: &str) -> Option<String> {
    let candidate = name.rsplit('/').next().unwrap_or(name);

    if candidate.is_empty()
        || candidate == "."
        || candidate == ".."
        || candidate.contains('/')
        || candidate.contains('\\')
        || candidate.contains(':')
        || candidate.contains('\0')
    {
        return None;
    }

    Some(candidate.to_string())
}

pub(in crate::linker::bins) fn sanitize_explicit_bin_name(name: &str) -> Option<String> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains('/')
        || name.contains('\\')
        || name.contains(':')
        || name.contains('\0')
    {
        return None;
    }

    Some(name.to_string())
}

pub(in crate::linker::bins) fn resolve_bin_target(root: &Path, script: &str) -> Option<PathBuf> {
    let script_path = Path::new(script);
    if script_path.is_absolute() {
        return None;
    }

    let mut target = root.to_path_buf();
    for component in script_path.components() {
        match component {
            Component::Normal(part) => target.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }

    Some(target)
}

#[cfg(test)]
mod tests {
    use super::{resolve_bin_target, sanitize_bin_name, sanitize_explicit_bin_name};
    use std::path::{Path, PathBuf};

    #[test]
    fn sanitize_bin_name_unscoped() {
        assert_eq!(
            sanitize_bin_name("typescript"),
            Some("typescript".to_string())
        );
    }

    #[test]
    fn sanitize_bin_name_scoped() {
        assert_eq!(sanitize_bin_name("@types/node"), Some("node".to_string()));
    }

    #[test]
    fn sanitize_bin_name_rejects_empty() {
        assert_eq!(sanitize_bin_name(""), None);
    }

    #[test]
    fn sanitize_bin_name_rejects_dot() {
        assert_eq!(sanitize_bin_name("."), None);
    }

    #[test]
    fn sanitize_bin_name_rejects_dotdot() {
        assert_eq!(sanitize_bin_name(".."), None);
    }

    #[test]
    fn sanitize_explicit_bin_name_valid() {
        assert_eq!(sanitize_explicit_bin_name("tsc"), Some("tsc".to_string()));
    }

    #[test]
    fn sanitize_explicit_bin_name_rejects_slash() {
        assert_eq!(sanitize_explicit_bin_name("foo/bar"), None);
    }

    #[test]
    fn sanitize_explicit_bin_name_rejects_backslash() {
        assert_eq!(sanitize_explicit_bin_name("foo\\bar"), None);
    }

    #[test]
    fn sanitize_explicit_bin_name_rejects_colon() {
        assert_eq!(sanitize_explicit_bin_name("foo:bar"), None);
    }

    #[test]
    fn sanitize_explicit_bin_name_rejects_null() {
        assert_eq!(sanitize_explicit_bin_name("foo\0bar"), None);
    }

    #[test]
    fn sanitize_explicit_bin_name_rejects_empty() {
        assert_eq!(sanitize_explicit_bin_name(""), None);
    }

    #[test]
    fn resolve_bin_target_simple() {
        let root = Path::new("/pkg");
        let result = resolve_bin_target(root, "bin/cli.js");
        assert_eq!(result, Some(PathBuf::from("/pkg/bin/cli.js")));
    }

    #[test]
    fn resolve_bin_target_with_curdir() {
        let root = Path::new("/pkg");
        let result = resolve_bin_target(root, "./bin/cli.js");
        assert_eq!(result, Some(PathBuf::from("/pkg/bin/cli.js")));
    }

    #[test]
    fn resolve_bin_target_rejects_absolute() {
        let root = Path::new("/pkg");
        assert_eq!(resolve_bin_target(root, "/etc/passwd"), None);
    }

    #[test]
    fn resolve_bin_target_rejects_parent_traversal() {
        let root = Path::new("/pkg");
        assert_eq!(resolve_bin_target(root, "../escape.js"), None);
    }
}
