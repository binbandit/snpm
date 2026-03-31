use crate::{Project, Result, SnpmError};

use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn collect_dir_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir)
        .map_err(|source| SnpmError::ReadFile {
            path: dir.to_path_buf(),
            source,
        })?
        .flatten()
    {
        let path = entry.path();
        if path.is_file() {
            files.push(path);
        } else if path.is_dir() {
            collect_dir_files(&path, files)?;
        }
    }

    Ok(())
}

pub(super) fn collect_dir_files_filtered(
    dir: &Path,
    files: &mut Vec<PathBuf>,
    project: &Project,
) -> Result<()> {
    let ignore_patterns = load_ignore_patterns(project);
    collect_dir_recursive(dir, files, &project.root, ignore_patterns.as_deref())
}

fn collect_dir_recursive(
    dir: &Path,
    files: &mut Vec<PathBuf>,
    root: &Path,
    ignore_patterns: Option<&[String]>,
) -> Result<()> {
    for entry in fs::read_dir(dir)
        .map_err(|source| SnpmError::ReadFile {
            path: dir.to_path_buf(),
            source,
        })?
        .flatten()
    {
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        if should_skip_path(&name_str) || should_ignore_path(&path, root, ignore_patterns) {
            continue;
        }

        if path.is_file() {
            files.push(path);
        } else if path.is_dir() {
            collect_dir_recursive(&path, files, root, ignore_patterns)?;
        }
    }

    Ok(())
}

fn load_ignore_patterns(project: &Project) -> Option<Vec<String>> {
    let npmignore_path = project.root.join(".npmignore");
    let gitignore_path = project.root.join(".gitignore");

    if npmignore_path.is_file() {
        return fs::read_to_string(&npmignore_path)
            .ok()
            .map(|content| parse_ignore_patterns(&content));
    }

    if gitignore_path.is_file() {
        return fs::read_to_string(&gitignore_path)
            .ok()
            .map(|content| parse_ignore_patterns(&content));
    }

    None
}

fn should_skip_path(name: &str) -> bool {
    matches!(
        name,
        "node_modules"
            | ".git"
            | ".snpm"
            | ".DS_Store"
            | "snpm-lock.yaml"
            | "pnpm-lock.yaml"
            | "package-lock.json"
    )
}

fn should_ignore_path(path: &Path, root: &Path, patterns: Option<&[String]>) -> bool {
    let Some(patterns) = patterns else {
        return false;
    };

    let relative_path = path.strip_prefix(root).unwrap_or(path).to_string_lossy();
    patterns
        .iter()
        .any(|pattern| matches_ignore_pattern(&relative_path, pattern))
}

fn parse_ignore_patterns(content: &str) -> Vec<String> {
    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(String::from)
        .collect()
}

fn matches_ignore_pattern(path: &str, pattern: &str) -> bool {
    let pattern = pattern.trim_end_matches('/');

    if !pattern.contains('/') {
        return path.split('/').any(|component| component == pattern);
    }

    path.starts_with(pattern) || path == pattern
}
