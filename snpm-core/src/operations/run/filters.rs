use crate::{Project, Result, SnpmError, Workspace};

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(test)]
pub(crate) fn matches_filters(name: &str, filters: &[String]) -> bool {
    if filters.is_empty() {
        return true;
    }

    for filter in filters {
        if filter == name {
            return true;
        }

        if let Ok(pattern) = glob::Pattern::new(filter) {
            if pattern.matches(name) {
                return true;
            }
        } else if name.contains(filter) {
            return true;
        }
    }

    false
}

pub fn select_workspace_projects<'a>(
    workspace: &'a Workspace,
    filters: &[String],
    filter_prods: &[String],
) -> Result<Vec<&'a Project>> {
    if filters.is_empty() && filter_prods.is_empty() {
        return Ok(workspace.projects.iter().collect());
    }

    let selectors = parse_effective_filters(filters, filter_prods).map_err(|reason| {
        SnpmError::WorkspaceConfig {
            path: workspace.root.clone(),
            reason,
        }
    })?;

    if selectors.is_empty() {
        return Ok(Vec::new());
    }

    let packages = index_packages(&workspace.projects);
    let graph = build_dependency_graph(&packages);
    let selected = apply_selectors(workspace.root.as_path(), &graph, &selectors)?;
    let mut matched = Vec::new();

    for (idx, project) in workspace.projects.iter().enumerate() {
        if selected.contains(&idx) {
            matched.push(project);
        }
    }

    Ok(matched)
}

fn apply_selectors(
    workspace_root: &Path,
    graph: &DependencyGraph<'_>,
    selectors: &[Selector],
) -> Result<BTreeSet<usize>> {
    let has_positive = selectors.iter().any(|selector| !selector.exclude);
    let mut included: BTreeSet<usize> = if has_positive {
        BTreeSet::new()
    } else {
        (0..graph.packages.len()).collect()
    };
    let mut excluded = BTreeSet::new();

    for selector in selectors {
        let matches = expand_selector(workspace_root, graph, selector)?;
        if selector.exclude {
            excluded.extend(matches);
        } else {
            included.extend(matches);
        }
    }

    for index in excluded {
        included.remove(&index);
    }

    Ok(included)
}

fn expand_selector(
    workspace_root: &Path,
    graph: &DependencyGraph<'_>,
    selector: &Selector,
) -> Result<BTreeSet<usize>> {
    let mut included = match &selector.base {
        BaseSelector::ChangedSince(rev) => changed_since(workspace_root, graph.packages, rev)?,
        BaseSelector::Name(_) | BaseSelector::NameGlob(_) | BaseSelector::Path(_) => {
            let mut seeds = BTreeSet::new();
            for (idx, package) in graph.packages.iter().enumerate() {
                if selector_matches(workspace_root, package, selector) {
                    seeds.insert(idx);
                }
            }
            seeds
        }
    };

    let original = included.clone();
    if selector.include_dependencies {
        included.extend(walk_dependencies(graph, &original, selector.prod_only));
    }
    if selector.include_dependents {
        included.extend(walk_dependents(graph, &original, selector.prod_only));
    }
    if selector.exclude_self {
        for index in original {
            included.remove(&index);
        }
    }

    Ok(included)
}

fn selector_matches(
    workspace_root: &Path,
    package: &IndexedPackage<'_>,
    selector: &Selector,
) -> bool {
    match &selector.base {
        BaseSelector::Name(name) => package.project.manifest.name.as_deref() == Some(name.as_str()),
        BaseSelector::NameGlob(pattern) => {
            if let Some(name) = package.project.manifest.name.as_deref() {
                pattern_matches(pattern, name)
            } else {
                false
            }
        }
        BaseSelector::Path(relative) => {
            let target = if relative.is_absolute() {
                relative.to_owned()
            } else {
                workspace_root.join(relative)
            };
            package.project.root.starts_with(target)
        }
        BaseSelector::ChangedSince(_) => false,
    }
}

fn parse_effective_filters(
    filters: &[String],
    filter_prods: &[String],
) -> std::result::Result<Vec<Selector>, String> {
    let mut selectors = Vec::with_capacity(filters.len() + filter_prods.len());
    for selector in filters {
        selectors.push(parse_selector(selector, false)?);
    }
    for selector in filter_prods {
        selectors.push(parse_selector(selector, true)?);
    }

    Ok(selectors)
}

fn parse_selector(raw: &str, prod_only: bool) -> std::result::Result<Selector, String> {
    if raw.is_empty() {
        return Err("empty --filter selector".to_string());
    }

    let (exclude, raw) = raw
        .strip_prefix('!')
        .map(|selector| (true, selector))
        .unwrap_or((false, raw));
    if raw.is_empty() {
        return Err("empty --filter selector".to_string());
    }

    let (include_dependents, raw) = raw
        .strip_prefix("...")
        .map(|selector| (true, selector))
        .unwrap_or((false, raw));
    let (exclude_self_from_dependents, raw) = raw
        .strip_prefix("^")
        .map(|selector| (true, selector))
        .unwrap_or((false, raw));
    let (include_dependencies, raw) = raw
        .strip_suffix("...")
        .map(|selector| (true, selector))
        .unwrap_or((false, raw));
    let (exclude_self_from_dependencies, raw) = raw
        .strip_suffix("^")
        .map(|selector| (true, selector))
        .unwrap_or((false, raw));
    if raw.is_empty() {
        return Err("empty --filter selector".to_string());
    }

    let includes_graph = include_dependencies || include_dependents;
    if (exclude_self_from_dependencies || exclude_self_from_dependents) && !includes_graph {
        return Err("selector '^' can only be used with dependency graph operators".to_string());
    }

    let base = if raw.starts_with('[') && raw.ends_with(']') && raw.len() > 2 {
        BaseSelector::ChangedSince(raw[1..raw.len() - 1].to_string())
    } else if raw.starts_with("./")
        || raw.starts_with(".\\")
        || raw.starts_with("../")
        || raw.starts_with("..\\")
        || raw.starts_with('/')
        || Path::new(raw).is_absolute()
        || raw.ends_with('\\')
        || raw.ends_with('/')
        || raw.contains('\\')
    {
        let normalized = normalize_selector_path(raw)?;
        BaseSelector::Path(normalized)
    } else if raw.contains('*') || raw.contains('?') {
        BaseSelector::NameGlob(raw.to_string())
    } else {
        BaseSelector::Name(raw.to_string())
    };

    Ok(Selector {
        base,
        include_dependencies,
        include_dependents,
        exclude_self: exclude_self_from_dependencies || exclude_self_from_dependents,
        exclude,
        prod_only,
    })
}

fn walk_dependencies(
    graph: &DependencyGraph<'_>,
    seeds: &BTreeSet<usize>,
    prod_only: bool,
) -> BTreeSet<usize> {
    let mut result = BTreeSet::new();
    let mut pending: VecDeque<usize> = seeds.iter().copied().collect();

    while let Some(current) = pending.pop_front() {
        let dependencies = if prod_only {
            &graph.outgoing_prod[current]
        } else {
            &graph.outgoing_all[current]
        };
        for &next in dependencies {
            if result.insert(next) {
                pending.push_back(next);
            }
        }
    }

    result
}

fn walk_dependents(
    graph: &DependencyGraph<'_>,
    seeds: &BTreeSet<usize>,
    prod_only: bool,
) -> BTreeSet<usize> {
    let mut result = BTreeSet::new();
    let mut pending: VecDeque<usize> = seeds.iter().copied().collect();
    while let Some(current) = pending.pop_front() {
        let dependents = if prod_only {
            &graph.incoming_prod[current]
        } else {
            &graph.incoming_all[current]
        };

        for &idx in dependents {
            if result.insert(idx) {
                pending.push_back(idx);
            }
        }
    }

    result
}

fn build_dependency_graph<'a>(packages: &'a [IndexedPackage<'a>]) -> DependencyGraph<'a> {
    let index = index_by_name(packages);
    let mut outgoing_all: Vec<Vec<usize>> = vec![Vec::new(); packages.len()];
    let mut outgoing_prod: Vec<Vec<usize>> = vec![Vec::new(); packages.len()];
    let mut incoming_all: Vec<Vec<usize>> = vec![Vec::new(); packages.len()];
    let mut incoming_prod: Vec<Vec<usize>> = vec![Vec::new(); packages.len()];

    for (from_idx, package) in packages.iter().enumerate() {
        for dependency in outgoing_dependencies(package, false) {
            let Some(&to_idx) = index.get(dependency.as_str()) else {
                continue;
            };
            outgoing_all[from_idx].push(to_idx);
            incoming_all[to_idx].push(from_idx);
        }

        for dependency in outgoing_dependencies(package, true) {
            let Some(&to_idx) = index.get(dependency.as_str()) else {
                continue;
            };
            outgoing_prod[from_idx].push(to_idx);
            incoming_prod[to_idx].push(from_idx);
        }
    }

    DependencyGraph {
        packages,
        outgoing_all,
        outgoing_prod,
        incoming_all,
        incoming_prod,
    }
}

fn normalize_selector_path(selector: &str) -> std::result::Result<PathBuf, String> {
    let relative = selector.trim_end_matches('/').trim_end_matches('\\');

    let normalized = relative
        .strip_prefix("./")
        .or_else(|| relative.strip_prefix(".\\"))
        .unwrap_or(relative);

    if normalized.is_empty() {
        return Err("empty path selector".to_string());
    }

    Ok(PathBuf::from(normalized))
}

fn outgoing_dependencies<'a>(
    package: &'a IndexedPackage<'a>,
    prod_only: bool,
) -> &'a BTreeSet<String> {
    if prod_only {
        &package.prod_dependencies
    } else {
        &package.all_dependencies
    }
}

fn index_by_name<'a>(packages: &'a [IndexedPackage<'a>]) -> BTreeMap<&'a str, usize> {
    let mut index = BTreeMap::new();
    for (idx, package) in packages.iter().enumerate() {
        if let Some(name) = package.project.manifest.name.as_deref() {
            index.insert(name, idx);
        }
    }
    index
}

fn changed_since(
    workspace_root: &Path,
    packages: &[IndexedPackage<'_>],
    revision: &str,
) -> Result<BTreeSet<usize>> {
    let mut command = Command::new("git");
    let output = command
        .arg("-C")
        .arg(workspace_root)
        .args(["diff", "--name-only", revision])
        .output()
        .map_err(|error| SnpmError::WorkspaceConfig {
            path: workspace_root.to_path_buf(),
            reason: format!("failed to run git diff: {error}"),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(SnpmError::WorkspaceConfig {
            path: workspace_root.to_path_buf(),
            reason: format!("git diff failed: {}", stderr.trim()),
        });
    }

    let changed: Vec<PathBuf> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_owned())
            }
        })
        .map(|line| workspace_root.join(line))
        .collect();

    let mut selected = BTreeSet::new();
    for (idx, package) in packages.iter().enumerate() {
        if changed
            .iter()
            .any(|path| path.starts_with(&package.project.root))
        {
            selected.insert(idx);
        }
    }

    Ok(selected)
}

#[derive(Debug)]
enum BaseSelector {
    ChangedSince(String),
    Path(PathBuf),
    NameGlob(String),
    Name(String),
}

#[derive(Debug)]
struct Selector {
    base: BaseSelector,
    include_dependencies: bool,
    include_dependents: bool,
    exclude_self: bool,
    exclude: bool,
    prod_only: bool,
}

fn pattern_matches(pattern: &str, value: &str) -> bool {
    if let Ok(glob_pattern) = glob::Pattern::new(pattern) {
        glob_pattern.matches(value)
    } else {
        value.contains(pattern)
    }
}

#[derive(Debug)]
struct DependencyGraph<'a> {
    packages: &'a [IndexedPackage<'a>],
    outgoing_all: Vec<Vec<usize>>,
    outgoing_prod: Vec<Vec<usize>>,
    incoming_all: Vec<Vec<usize>>,
    incoming_prod: Vec<Vec<usize>>,
}

pub fn format_filters(filters: &[String], filter_prods: &[String]) -> String {
    if filters.is_empty() && filter_prods.is_empty() {
        return "--recursive".to_string();
    }

    let mut rendered = Vec::with_capacity(filters.len() + filter_prods.len());
    for selector in filters {
        rendered.push(format!("--filter {selector}"));
    }
    for selector in filter_prods {
        rendered.push(format!("--filter-prod {selector}"));
    }
    rendered.join(", ")
}

#[derive(Debug)]
struct IndexedPackage<'a> {
    project: &'a Project,
    all_dependencies: BTreeSet<String>,
    prod_dependencies: BTreeSet<String>,
}

pub fn project_label(project: &Project) -> String {
    if let Some(name) = project.manifest.name.as_deref() {
        return name.to_string();
    }

    project
        .root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(".")
        .to_string()
}

fn index_packages(projects: &[Project]) -> Vec<IndexedPackage<'_>> {
    let mut indexed = Vec::with_capacity(projects.len());
    for project in projects {
        let all_dependencies = project
            .manifest
            .dependencies
            .keys()
            .chain(project.manifest.dev_dependencies.keys())
            .chain(project.manifest.optional_dependencies.keys())
            .cloned()
            .collect();
        let prod_dependencies = project
            .manifest
            .dependencies
            .keys()
            .chain(project.manifest.optional_dependencies.keys())
            .cloned()
            .collect();

        indexed.push(IndexedPackage {
            project,
            all_dependencies,
            prod_dependencies,
        });
    }

    indexed
}

#[cfg(test)]
mod tests {
    use super::{matches_filters, project_label};
    use crate::project::Manifest;
    use crate::workspace::types::WorkspaceConfig;
    use crate::{Project, Workspace};
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn matches_filters_empty_returns_true() {
        assert!(matches_filters("anything", &[]));
    }

    #[test]
    fn matches_filters_exact_match() {
        let filters = vec!["my-pkg".to_string()];
        assert!(matches_filters("my-pkg", &filters));
        assert!(!matches_filters("other-pkg", &filters));
    }

    #[test]
    fn matches_filters_glob_match() {
        let filters = vec!["@scope/*".to_string()];
        assert!(matches_filters("@scope/foo", &filters));
        assert!(!matches_filters("other", &filters));
    }

    #[test]
    fn matches_filters_substring_fallback() {
        let filters = vec!["[invalid".to_string()];
        assert!(matches_filters("contains-[invalid-here", &filters));
        assert!(!matches_filters("no-match", &filters));
    }

    #[test]
    fn matches_filters_multiple() {
        let filters = vec!["pkg-a".to_string(), "pkg-b".to_string()];
        assert!(matches_filters("pkg-a", &filters));
        assert!(matches_filters("pkg-b", &filters));
        assert!(!matches_filters("pkg-c", &filters));
    }

    #[test]
    fn project_label_uses_name() {
        let dir = tempdir().unwrap();
        let project = Project {
            root: dir.path().to_path_buf(),
            manifest_path: dir.path().join("package.json"),
            manifest: Manifest {
                name: Some("my-project".to_string()),
                version: None,
                private: false,
                dependencies: BTreeMap::new(),
                dev_dependencies: BTreeMap::new(),
                optional_dependencies: BTreeMap::new(),
                scripts: BTreeMap::new(),
                resolutions: BTreeMap::new(),
                files: None,
                bin: None,
                main: None,
                pnpm: None,
                snpm: None,
                workspaces: None,
            },
        };

        assert_eq!(project_label(&project), "my-project");
    }

    #[test]
    fn project_label_falls_back_to_dir_name() {
        let dir = tempdir().unwrap();
        let project = Project {
            root: dir.path().to_path_buf(),
            manifest_path: dir.path().join("package.json"),
            manifest: Manifest {
                name: None,
                version: None,
                private: false,
                dependencies: BTreeMap::new(),
                dev_dependencies: BTreeMap::new(),
                optional_dependencies: BTreeMap::new(),
                scripts: BTreeMap::new(),
                resolutions: BTreeMap::new(),
                files: None,
                bin: None,
                main: None,
                pnpm: None,
                snpm: None,
                workspaces: None,
            },
        };

        let label = project_label(&project);
        assert!(!label.is_empty());
    }

    #[test]
    fn select_workspace_projects_with_graph_filters() {
        let temp = tempdir().unwrap();
        let root = temp.path().to_path_buf();
        let workspace = Workspace {
            root,
            config: WorkspaceConfig {
                packages: vec![],
                catalog: BTreeMap::new(),
                catalogs: BTreeMap::new(),
                only_built_dependencies: vec![],
                ignored_built_dependencies: vec![],
                hoisting: None,
            },
            projects: vec![
                Project {
                    root: PathBuf::from("/ws/packages/api"),
                    manifest_path: PathBuf::from("/ws/packages/api/package.json"),
                    manifest: Manifest {
                        name: Some("api".to_string()),
                        version: None,
                        private: false,
                        dependencies: BTreeMap::from([("lib".to_string(), "1".to_string())]),
                        dev_dependencies: BTreeMap::new(),
                        optional_dependencies: BTreeMap::new(),
                        scripts: BTreeMap::new(),
                        resolutions: BTreeMap::new(),
                        files: None,
                        bin: None,
                        main: None,
                        pnpm: None,
                        snpm: None,
                        workspaces: None,
                    },
                },
                Project {
                    root: PathBuf::from("/ws/packages/lib"),
                    manifest_path: PathBuf::from("/ws/packages/lib/package.json"),
                    manifest: Manifest {
                        name: Some("lib".to_string()),
                        version: None,
                        private: false,
                        dependencies: BTreeMap::new(),
                        dev_dependencies: BTreeMap::new(),
                        optional_dependencies: BTreeMap::new(),
                        scripts: BTreeMap::new(),
                        resolutions: BTreeMap::new(),
                        files: None,
                        bin: None,
                        main: None,
                        pnpm: None,
                        snpm: None,
                        workspaces: None,
                    },
                },
            ],
        };

        let selected =
            super::select_workspace_projects(&workspace, &vec!["api...".to_string()], &[]).unwrap();
        assert_eq!(selected.len(), 2);
        let names: BTreeSet<String> = selected
            .into_iter()
            .map(|project| project_label(project))
            .collect();
        assert!(names.contains("api"));
        assert!(names.contains("lib"));
    }
}
