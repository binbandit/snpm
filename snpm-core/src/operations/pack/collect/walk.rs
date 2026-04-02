use crate::{Project, Result, SnpmError};

use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use glob::Pattern;

use super::PackFileReason;
use super::{CollectedFile, add_file};

#[derive(Debug, Clone)]
struct IgnoreRule {
    base: String,
    matcher: Pattern,
    negated: bool,
    directory_only: bool,
    component_only: bool,
}

pub(super) fn collect_dir_files_filtered(
    dir: &Path,
    files: &mut BTreeMap<PathBuf, CollectedFile>,
    project: &Project,
    honor_root_ignore: bool,
) -> Result<()> {
    collect_dir_recursive(
        dir,
        files,
        &project.root,
        honor_root_ignore,
        PackFileReason::DefaultScan,
        &[],
    )
}

pub(super) fn collect_dir_files_filtered_from_root(
    dir: &Path,
    files: &mut BTreeMap<PathBuf, CollectedFile>,
    root: &Path,
    honor_root_ignore: bool,
    reason: PackFileReason,
) -> Result<()> {
    collect_dir_recursive(dir, files, root, honor_root_ignore, reason, &[])
}

pub(super) fn should_include_file_from_root(
    path: &Path,
    root: &Path,
    honor_root_ignore: bool,
) -> Result<bool> {
    let relative = relative_path(root, path);
    let relative_path = Path::new(&relative);
    if should_skip_path(relative_path) {
        return Ok(false);
    }

    let parent = path.parent().unwrap_or(root);
    let rules = collect_rules_for_dir(parent, root, honor_root_ignore)?;
    Ok(!is_ignored(relative_path, false, &rules))
}

fn collect_dir_recursive(
    dir: &Path,
    files: &mut BTreeMap<PathBuf, CollectedFile>,
    root: &Path,
    honor_root_ignore: bool,
    reason: PackFileReason,
    inherited_rules: &[IgnoreRule],
) -> Result<()> {
    let dir_relative = relative_path(root, dir);
    let mut rules = inherited_rules.to_vec();
    let honor_current_ignore = honor_root_ignore || dir != root;
    rules.extend(load_ignore_rules(dir, &dir_relative, honor_current_ignore)?);

    for entry in fs::read_dir(dir)
        .map_err(|source| SnpmError::ReadFile {
            path: dir.to_path_buf(),
            source,
        })?
        .flatten()
    {
        let path = entry.path();
        let file_type = entry.file_type().map_err(|source| SnpmError::ReadFile {
            path: path.clone(),
            source,
        })?;
        let relative = relative_path(root, &path);
        let relative_path = Path::new(&relative);

        if should_skip_path(relative_path) || file_type.is_symlink() {
            continue;
        }

        if file_type.is_dir() {
            collect_dir_recursive(&path, files, root, true, reason.clone(), &rules)?;
            continue;
        }

        if !file_type.is_file() || is_ignored(relative_path, false, &rules) {
            continue;
        }

        add_file(root, &path, reason.clone(), files)?;
    }

    Ok(())
}

fn collect_rules_for_dir(
    dir: &Path,
    root: &Path,
    honor_root_ignore: bool,
) -> Result<Vec<IgnoreRule>> {
    let mut chain = Vec::new();
    let mut current = Some(dir);

    while let Some(path) = current {
        chain.push(path.to_path_buf());
        if path == root {
            break;
        }
        current = path.parent();
    }

    chain.reverse();

    let mut rules = Vec::new();
    for path in chain {
        let relative = relative_path(root, &path);
        let honor_ignore = if path == root {
            honor_root_ignore
        } else {
            true
        };
        rules.extend(load_ignore_rules(&path, &relative, honor_ignore)?);
    }

    Ok(rules)
}

fn load_ignore_rules(
    dir: &Path,
    dir_relative: &str,
    honor_ignore: bool,
) -> Result<Vec<IgnoreRule>> {
    if !honor_ignore {
        return Ok(Vec::new());
    }

    let npmignore_path = dir.join(".npmignore");
    let gitignore_path = dir.join(".gitignore");
    let ignore_path = if npmignore_path.is_file() {
        Some(npmignore_path)
    } else if gitignore_path.is_file() {
        Some(gitignore_path)
    } else {
        None
    };

    let Some(ignore_path) = ignore_path else {
        return Ok(Vec::new());
    };

    let content = fs::read_to_string(&ignore_path).map_err(|source| SnpmError::ReadFile {
        path: ignore_path,
        source,
    })?;

    Ok(parse_ignore_rules(&content, dir_relative))
}

fn parse_ignore_rules(content: &str, dir_relative: &str) -> Vec<IgnoreRule> {
    let base = trim_slashes(dir_relative);

    content
        .lines()
        .filter_map(|line| parse_ignore_rule(line, &base))
        .collect()
}

fn parse_ignore_rule(line: &str, base: &str) -> Option<IgnoreRule> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }

    let (negated, body) = match trimmed.strip_prefix('!') {
        Some(pattern) => (true, pattern.trim()),
        None => (false, trimmed),
    };

    let directory_only = body.ends_with('/');
    let body = trim_slashes(body);
    if body.is_empty() {
        return None;
    }

    let component_only = !body.contains('/');
    let pattern = if component_only {
        body.to_string()
    } else {
        body.trim_start_matches('/').to_string()
    };
    let matcher = Pattern::new(&pattern).ok()?;

    Some(IgnoreRule {
        base: base.to_string(),
        matcher,
        negated,
        directory_only,
        component_only,
    })
}

fn is_ignored(path: &Path, is_dir: bool, rules: &[IgnoreRule]) -> bool {
    let candidate = normalize_path(path);
    let mut ignored = false;

    for rule in rules {
        if rule_matches(rule, &candidate, is_dir) {
            ignored = !rule.negated;
        }
    }

    ignored
}

fn rule_matches(rule: &IgnoreRule, candidate: &str, is_dir: bool) -> bool {
    if rule.directory_only {
        return if is_dir {
            rule_matches_path(rule, candidate)
        } else {
            directory_rule_matches_file(rule, candidate)
        };
    }

    rule_matches_path(rule, candidate)
}

fn rule_matches_path(rule: &IgnoreRule, candidate: &str) -> bool {
    let Some(relative) = strip_base(candidate, &rule.base) else {
        return false;
    };

    if rule.component_only {
        for component in Path::new(relative).components() {
            let Component::Normal(component) = component else {
                continue;
            };
            let component = component.to_string_lossy();
            if rule.matcher.matches(&component) {
                return true;
            }
        }

        return false;
    }

    rule.matcher.matches(relative)
}

fn directory_rule_matches_file(rule: &IgnoreRule, candidate: &str) -> bool {
    let Some(relative) = strip_base(candidate, &rule.base) else {
        return false;
    };

    let path = Path::new(relative);
    for ancestor in path.ancestors().skip(1) {
        if ancestor.as_os_str().is_empty() {
            continue;
        }

        let ancestor = normalize_path(ancestor);
        if rule_matches_path(rule, &join_base(&rule.base, &ancestor)) {
            return true;
        }
    }

    false
}

fn strip_base<'a>(candidate: &'a str, base: &str) -> Option<&'a str> {
    if base.is_empty() {
        return Some(candidate);
    }

    candidate
        .strip_prefix(base)
        .and_then(|rest| rest.strip_prefix('/'))
}

fn join_base(base: &str, path: &str) -> String {
    if base.is_empty() {
        path.to_string()
    } else {
        format!("{base}/{path}")
    }
}

fn should_skip_path(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };

    if matches!(
        name,
        "node_modules"
            | ".git"
            | ".hg"
            | ".svn"
            | "CVS"
            | ".snpm"
            | ".DS_Store"
            | ".npmrc"
            | "snpm-lock.yaml"
            | "pnpm-lock.yaml"
            | "package-lock.json"
            | "yarn.lock"
            | "bun.lockb"
            | "npm-debug.log"
            | "config.gypi"
            | ".lock-wscript"
    ) {
        return true;
    }

    name.ends_with(".orig")
        || (name.starts_with('.') && name.ends_with(".swp"))
        || name.starts_with("._")
        || name.starts_with(".wafpickle-")
}

fn relative_path(root: &Path, path: &Path) -> String {
    normalize_path(path.strip_prefix(root).unwrap_or(path))
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn trim_slashes(value: &str) -> &str {
    value.trim_matches('/')
}

#[cfg(test)]
mod tests {
    use super::{IgnoreRule, is_ignored, parse_ignore_rule};

    use std::path::Path;

    fn rule(pattern: &str) -> IgnoreRule {
        parse_ignore_rule(pattern, "").unwrap()
    }

    #[test]
    fn directory_rule_matches_nested_files() {
        let rules = vec![rule("dist/")];

        assert!(is_ignored(Path::new("dist/app.js"), false, &rules));
        assert!(is_ignored(Path::new("dist/nested/app.js"), false, &rules));
    }

    #[test]
    fn negated_rule_reincludes_specific_file() {
        let rules = vec![rule("dist/"), rule("!dist/keep.js")];

        assert!(is_ignored(Path::new("dist/other.js"), false, &rules));
        assert!(!is_ignored(Path::new("dist/keep.js"), false, &rules));
    }

    #[test]
    fn component_rule_matches_any_segment() {
        let rules = vec![rule("fixtures")];

        assert!(is_ignored(
            Path::new("src/fixtures/data.json"),
            false,
            &rules
        ));
        assert!(is_ignored(Path::new("fixtures/data.json"), false, &rules));
        assert!(!is_ignored(
            Path::new("src/fixture-data.json"),
            false,
            &rules
        ));
    }

    #[test]
    fn nested_rule_base_is_respected() {
        let rule = parse_ignore_rule("generated/**", "dist").unwrap();
        let rules = vec![rule];

        assert!(is_ignored(
            Path::new("dist/generated/app.js"),
            false,
            &rules
        ));
        assert!(!is_ignored(Path::new("generated/app.js"), false, &rules));
    }
}
