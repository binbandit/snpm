use std::path::{Component, Path};

pub(super) fn rewrite_diff_paths(
    content: &str,
    original_root: &Path,
    modified_root: &Path,
) -> String {
    let mut rewritten = String::new();

    for line in content.lines() {
        let line = rewrite_patch_header(line, "--- ", original_root, "a")
            .or_else(|| rewrite_patch_header(line, "+++ ", modified_root, "b"))
            .unwrap_or_else(|| line.to_string());
        rewritten.push_str(&line);
        rewritten.push('\n');
    }

    rewritten
}

fn rewrite_patch_header(line: &str, marker: &str, root: &Path, prefix: &str) -> Option<String> {
    let rest = line.strip_prefix(marker)?;
    if rest == "/dev/null" || rest.starts_with("/dev/null\t") {
        return Some(line.to_string());
    }

    let (path_text, suffix) = match rest.split_once('\t') {
        Some((path_text, suffix)) => (path_text, Some(suffix)),
        None => (rest, None),
    };

    let relative = Path::new(path_text).strip_prefix(root).ok()?;
    let mut rewritten = format!("{marker}{prefix}/{}", normalize_patch_path(relative));

    if let Some(suffix) = suffix {
        rewritten.push('\t');
        rewritten.push_str(suffix);
    }

    Some(rewritten)
}

pub(super) fn normalize_patch_path(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}
