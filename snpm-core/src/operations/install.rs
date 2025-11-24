use crate::console;
use crate::lifecycle;
use crate::lockfile;
use crate::registry::RegistryProtocol;
use crate::resolve::ResolutionGraph;
use crate::workspace::CatalogConfig;
use crate::workspace::OverridesConfig;
use crate::{Project, Result, SnpmConfig, SnpmError, Workspace, linker, resolve, store};
use futures::future::join_all;
use reqwest::Client;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct InstallOptions {
    pub requested: Vec<String>,
    pub dev: bool,
    pub include_dev: bool,
    pub frozen_lockfile: bool,
    pub force: bool,
}

#[derive(Debug)]
pub struct OutdatedEntry {
    pub name: String,
    pub current: Option<String>,
    pub wanted: String,
}

#[derive(Debug, Clone)]
struct ParsedSpec {
    name: String,
    range: String,
    protocol: Option<String>,
}

pub async fn install(
    config: &SnpmConfig,
    project: &mut Project,
    options: InstallOptions,
) -> Result<()> {
    let started = Instant::now();
    let (requested_ranges, requested_protocols) = parse_requested_with_protocol(&options.requested);

    let additions = requested_ranges;

    let workspace = Workspace::discover(&project.root)?;

    console::project(&project_label(project));

    let catalog = if workspace.is_none() {
        CatalogConfig::load(&project.root)?
    } else {
        None
    };

    let overrides_from_file = if let Some(ws) = workspace.as_ref() {
        OverridesConfig::load(&ws.root)?
            .map(|c| c.overrides)
            .unwrap_or_default()
    } else {
        OverridesConfig::load(&project.root)?
            .map(|c| c.overrides)
            .unwrap_or_default()
    };

    let mut overrides = overrides_from_file;

    if let Some(pnpm) = &project.manifest.pnpm {
        for (name, range) in pnpm.overrides.iter() {
            overrides.insert(name.clone(), range.clone());
        }
    }

    if let Some(snpm) = &project.manifest.snpm {
        for (name, range) in snpm.overrides.iter() {
            overrides.insert(name.clone(), range.clone());
        }
    }

    let mut local_deps = BTreeSet::new();
    let mut local_dev_deps = BTreeSet::new();
    let mut manifest_protocols = BTreeMap::new();

    let deps = apply_specs(
        &project.manifest.dependencies,
        workspace.as_ref(),
        &mut local_deps,
        Some(&mut manifest_protocols),
    )?;
    let dev_deps = apply_specs(
        &project.manifest.dev_dependencies,
        workspace.as_ref(),
        &mut local_dev_deps,
        Some(&mut manifest_protocols),
    )?;

    let manifest_root = if let Some(ws) = workspace.as_ref() {
        collect_workspace_root_deps(ws, options.include_dev)?
    } else {
        build_project_manifest_root(&deps, &dev_deps, options.include_dev)
    };

    let mut root_deps = manifest_root.clone();

    for (name, range) in additions.iter() {
        root_deps.insert(name.clone(), range.clone());
    }

    let mut root_protocols = BTreeMap::new();

    for name in manifest_root.keys() {
        if let Some(proto) = manifest_protocols.get(name) {
            root_protocols.insert(name.clone(), proto.clone());
        } else {
            root_protocols.insert(name.clone(), RegistryProtocol::npm());
        }
    }

    for name in additions.keys() {
        if let Some(proto) = requested_protocols.get(name) {
            root_protocols.insert(name.clone(), proto.clone());
        } else {
            root_protocols
                .entry(name.clone())
                .or_insert_with(RegistryProtocol::npm);
        }
    }

    let lockfile_path = workspace
        .as_ref()
        .map(|w| w.root.join("snpm-lock.yaml"))
        .unwrap_or_else(|| project.root.join("snpm-lock.yaml"));

    let graph = if options.frozen_lockfile {
        if !lockfile_path.is_file() {
            return Err(SnpmError::Lockfile {
                path: lockfile_path.clone(),
                reason: "frozen-lockfile requested but snpm-lock.yaml is missing".into(),
            });
        }

        if !additions.is_empty() {
            return Err(SnpmError::Lockfile {
                path: lockfile_path.clone(),
                reason: "cannot add packages when using frozen-lockfile".into(),
            });
        }

        let existing = lockfile::read(&lockfile_path)?;
        let mut lock_requested = BTreeMap::new();

        for (name, dep) in existing.root.dependencies.iter() {
            lock_requested.insert(name.clone(), dep.requested.clone());
        }

        if lock_requested != manifest_root {
            return Err(SnpmError::Lockfile {
                path: lockfile_path.clone(),
                reason:
                    "manifest dependencies do not match snpm-lock.yaml when using frozen-lockfile"
                        .into(),
            });
        }

        lockfile::to_graph(&existing)
    } else {
        let can_reuse_lockfile =
            options.include_dev && additions.is_empty() && lockfile_path.is_file();

        if can_reuse_lockfile {
            let existing = lockfile::read(&lockfile_path)?;
            let mut lock_requested = BTreeMap::new();

            for (name, dep) in existing.root.dependencies.iter() {
                lock_requested.insert(name.clone(), dep.requested.clone());
            }

            if lock_requested == manifest_root {
                lockfile::to_graph(&existing)
            } else {
                let graph = resolve::resolve(
                    config,
                    &root_deps,
                    &root_protocols,
                    config.min_package_age_days,
                    options.force,
                    Some(&overrides),
                )
                .await?;
                lockfile::write(&lockfile_path, &graph)?;
                graph
            }
        } else {
            let graph = resolve::resolve(
                config,
                &root_deps,
                &root_protocols,
                config.min_package_age_days,
                options.force,
                Some(&overrides),
            )
            .await?;
            if options.include_dev {
                lockfile::write(&lockfile_path, &graph)?;
            }
            graph
        }
    };

    console::step("resolved", &format!("{} packages", graph.packages.len()));

    let store_paths = materialize_store(config, &graph).await?;
    console::step("fetched", &format!("{} packages", store_paths.len()));

    write_manifest(
        project,
        &graph,
        &additions,
        options.dev,
        workspace.as_ref(),
        catalog.as_ref(),
    )?;
    linker::link(
        config,
        workspace.as_ref(),
        project,
        &graph,
        &store_paths,
        options.include_dev,
    )?;
    link_local_workspace_deps(
        project,
        workspace.as_ref(),
        &local_deps,
        &local_dev_deps,
        options.include_dev,
    )?;
    console::step("linked", "node_modules");
    lifecycle::run_install_scripts(config, workspace.as_ref(), &project.root)?;
    console::step("scripts", "install scripts completed");
    if !additions.is_empty() {
        for name in additions.keys() {
            if let Some(dep) = graph.root.dependencies.get(name) {
                let mut summary = format!("{}@{}", name, dep.resolved.version);
                if options.dev {
                    summary.push_str(" (dev)");
                }
                console::added(&summary);
            }
        }
    }

    let elapsed = started.elapsed();
    let seconds = elapsed.as_secs_f32();
    console::step("done", &format!("in {seconds:.2}s"));

    Ok(())
}

pub async fn remove(config: &SnpmConfig, project: &mut Project, specs: Vec<String>) -> Result<()> {
    if specs.is_empty() {
        return Ok(());
    }

    let mut manifest = project.manifest.clone();

    for spec in specs {
        let (name, _) = parse_spec(&spec);

        let mut removed_any = false;

        if manifest.dependencies.remove(&name).is_some() {
            removed_any = true;
        }

        if manifest.dev_dependencies.remove(&name).is_some() {
            removed_any = true;
        }

        if removed_any {
            crate::console::removed(&name);
        }
    }

    project.write_manifest(&manifest)?;
    project.manifest = manifest;

    let options = InstallOptions {
        requested: Vec::new(),
        dev: false,
        include_dev: true,
        frozen_lockfile: false,
        force: false,
    };

    install(config, project, options).await
}

fn parse_requested_spec(spec: &str) -> ParsedSpec {
    let mut protocol = None;
    let mut rest = spec;

    if let Some(idx) = spec.find(':') {
        let (prefix, after) = spec.split_at(idx);
        if !prefix.is_empty() {
            protocol = Some(prefix.to_string());
            rest = &after[1..];
        }
    }

    if rest.starts_with('@') {
        let without_at = &rest[1..];
        if let Some(idx) = without_at.rfind('@') {
            let (scope_and_name, range) = without_at.split_at(idx);
            let name = format!("@{}", scope_and_name);
            let requested = range.trim_start_matches('@').to_string();
            return ParsedSpec {
                name,
                range: requested,
                protocol,
            };
        } else {
            return ParsedSpec {
                name: rest.to_string(),
                range: "latest".to_string(),
                protocol,
            };
        }
    }

    if let Some(idx) = rest.rfind('@') {
        let (name, range) = rest.split_at(idx);
        ParsedSpec {
            name: name.to_string(),
            range: range.trim_start_matches('@').to_string(),
            protocol,
        }
    } else {
        ParsedSpec {
            name: rest.to_string(),
            range: "latest".to_string(),
            protocol,
        }
    }
}

fn parse_spec(spec: &str) -> (String, String) {
    if spec.starts_with('@') {
        let without_at = &spec[1..];
        if let Some(idx) = without_at.rfind('@') {
            let (scope_and_name, range) = without_at.split_at(idx);
            let name = format!("@{}", scope_and_name);
            let requested = range.trim_start_matches('@').to_string();
            return (name, requested);
        } else {
            return (spec.to_string(), "latest".to_string());
        }
    }

    if let Some(idx) = spec.rfind('@') {
        let (name, range) = spec.split_at(idx);
        let requested = range.trim_start_matches('@').to_string();
        (name.to_string(), requested)
    } else {
        (spec.to_string(), "latest".to_string())
    }
}

async fn materialize_store(
    config: &SnpmConfig,
    graph: &ResolutionGraph,
) -> Result<BTreeMap<crate::resolve::PackageId, PathBuf>> {
    let client = Client::new();
    let mut futures = Vec::new();

    for package in graph.packages.values() {
        let config = config.clone();
        let client = client.clone();

        let future = async move {
            let path = store::ensure_package(&config, &package, &client).await?;
            let id = package.id.clone();
            Ok::<(crate::resolve::PackageId, PathBuf), crate::SnpmError>((id, path))
        };

        futures.push(future);
    }

    let results = join_all(futures).await;
    let mut paths = BTreeMap::new();

    for result in results {
        let (id, path) = result?;
        paths.insert(id, path);
    }

    Ok(paths)
}

fn write_manifest(
    project: &mut Project,
    graph: &ResolutionGraph,
    additions: &BTreeMap<String, String>,
    dev: bool,
    workspace: Option<&Workspace>,
    catalog: Option<&CatalogConfig>,
) -> Result<()> {
    if additions.is_empty() {
        return Ok(());
    }

    let mut new_dependencies = project.manifest.dependencies.clone();
    let mut new_dev_dependencies = project.manifest.dev_dependencies.clone();

    for (name, dep) in graph.root.dependencies.iter() {
        if !additions.contains_key(name) {
            continue;
        }

        let mut use_catalog = false;

        if let Some(ws) = workspace {
            if ws.config.catalog.contains_key(name) {
                use_catalog = true;
            }
        }

        if !use_catalog {
            if let Some(cat) = catalog {
                if cat.catalog.contains_key(name) {
                    use_catalog = true;
                }
            }
        }

        let spec = if use_catalog {
            "catalog:".to_string()
        } else {
            format!("^{}", dep.resolved.version)
        };

        if dev {
            new_dev_dependencies.insert(name.clone(), spec);
        } else {
            new_dependencies.insert(name.clone(), spec);
        }
    }

    project.manifest.dependencies = new_dependencies;
    project.manifest.dev_dependencies = new_dev_dependencies;

    project.write_manifest(&project.manifest)?;

    Ok(())
}

fn build_project_manifest_root(
    deps: &BTreeMap<String, String>,
    dev_deps: &BTreeMap<String, String>,
    include_dev: bool,
) -> BTreeMap<String, String> {
    let mut root = deps.clone();

    if include_dev {
        for (name, range) in dev_deps.iter() {
            root.entry(name.clone()).or_insert(range.clone());
        }
    }

    root
}

fn collect_workspace_root_deps(
    workspace: &Workspace,
    include_dev: bool,
) -> Result<BTreeMap<String, String>> {
    let mut combined = BTreeMap::new();

    for member in workspace.projects.iter() {
        let mut local = BTreeSet::new();
        let deps = apply_specs(
            &member.manifest.dependencies,
            Some(workspace),
            &mut local,
            None,
        )?;

        for (name, range) in deps.iter() {
            insert_workspace_root_dep(&mut combined, &workspace.root, name, range)?;
        }

        if include_dev {
            let mut local_dev = BTreeSet::new();
            let dev_deps = apply_specs(
                &member.manifest.dev_dependencies,
                Some(workspace),
                &mut local_dev,
                None,
            )?;

            for (name, range) in dev_deps.iter() {
                insert_workspace_root_dep(&mut combined, &workspace.root, name, range)?;
            }
        }
    }

    Ok(combined)
}

fn insert_workspace_root_dep(
    combined: &mut BTreeMap<String, String>,
    root: &Path,
    name: &str,
    range: &str,
) -> Result<()> {
    if let Some(existing) = combined.get(name) {
        if existing != range {
            return Err(SnpmError::WorkspaceConfig {
                path: root.to_path_buf(),
                reason: format!(
                    "dependency {name} has conflicting ranges {existing} and {range} across workspace projects"
                ),
            });
        }
    } else {
        combined.insert(name.to_string(), range.to_string());
    }

    Ok(())
}

fn apply_specs(
    specs: &BTreeMap<String, String>,
    workspace: Option<&Workspace>,
    local_set: &mut BTreeSet<String>,
    mut protocol_map: Option<&mut BTreeMap<String, RegistryProtocol>>,
) -> Result<BTreeMap<String, String>> {
    let mut result = BTreeMap::new();

    for (name, value) in specs.iter() {
        if value.starts_with("workspace:") {
            local_set.insert(name.clone());
            continue;
        }

        let resolved = if value.starts_with("catalog:") {
            let ws = workspace.ok_or_else(|| SnpmError::WorkspaceConfig {
                path: PathBuf::from("."),
                reason: "catalog protocol used but no workspace configuration found".into(),
            })?;
            resolve_catalog_spec(name, value, ws)?
        } else {
            value.clone()
        };

        let final_value = if let Some(map) = &mut protocol_map {
            if let Some((range, proto)) = parse_manifest_protocol(&resolved) {
                map.insert(name.clone(), proto);
                range
            } else {
                resolved
            }
        } else {
            resolved
        };

        result.insert(name.clone(), final_value);
    }

    Ok(result)
}

fn resolve_catalog_spec(name: &str, value: &str, workspace: &Workspace) -> Result<String> {
    let selector = &value["catalog:".len()..];
    let cfg = &workspace.config;

    let range_opt = if selector.is_empty() || selector == "default" {
        cfg.catalog.get(name)
    } else {
        cfg.catalogs
            .get(selector)
            .and_then(|catalog| catalog.get(name))
    };

    match range_opt {
        Some(range) => Ok(range.clone()),
        None => Err(SnpmError::WorkspaceConfig {
            path: workspace.root.clone(),
            reason: format!("no catalog entry found for dependency {name} and selector {value}"),
        }),
    }
}

fn link_local_workspace_deps(
    project: &Project,
    workspace: Option<&Workspace>,
    local_deps: &BTreeSet<String>,
    local_dev_deps: &BTreeSet<String>,
    include_dev: bool,
) -> Result<()> {
    if local_deps.is_empty() && local_dev_deps.is_empty() {
        return Ok(());
    }

    let ws = match workspace {
        Some(ws) => ws,
        None => {
            return Err(SnpmError::WorkspaceConfig {
                path: project.root.clone(),
                reason: "workspace: protocol used but no workspace configuration found".into(),
            });
        }
    };

    let node_modules = project.root.join("node_modules");

    for name in local_deps.iter().chain(local_dev_deps.iter()) {
        let only_dev = local_dev_deps.contains(name) && !local_deps.contains(name);
        if !include_dev && only_dev {
            continue;
        }

        let source_project = ws
            .projects
            .iter()
            .find(|p| p.manifest.name.as_deref() == Some(name.as_str()))
            .ok_or_else(|| SnpmError::WorkspaceConfig {
                path: ws.root.clone(),
                reason: format!("workspace dependency {name} not found in workspace projects"),
            })?;

        let dest = node_modules.join(name);

        if dest.exists() {
            if dest.is_dir() {
                fs::remove_dir_all(&dest)
            } else {
                fs::remove_file(&dest)
            }
            .map_err(|source| SnpmError::WriteFile {
                path: dest.clone(),
                source,
            })?;
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            symlink(&source_project.root, &dest).map_err(|source| SnpmError::WriteFile {
                path: dest.clone(),
                source,
            })?;
        }

        #[cfg(windows)]
        {
            use std::os::windows::fs::symlink_dir;
            symlink_dir(&source_project.root, &dest).map_err(|source| SnpmError::WriteFile {
                path: dest.clone(),
                source,
            })?;
        }
    }

    Ok(())
}

pub async fn outdated(
    config: &SnpmConfig,
    project: &Project,
    include_dev: bool,
) -> Result<Vec<OutdatedEntry>> {
    let workspace = Workspace::discover(&project.root)?;

    let overrides = if let Some(ref ws) = workspace {
        match OverridesConfig::load(&ws.root)? {
            Some(cfg) => cfg.overrides,
            None => BTreeMap::new(),
        }
    } else {
        match OverridesConfig::load(&project.root)? {
            Some(cfg) => cfg.overrides,
            None => BTreeMap::new(),
        }
    };

    let mut local_deps = BTreeSet::new();
    let mut local_dev_deps = BTreeSet::new();
    let mut manifest_protocols = BTreeMap::new();

    let deps = apply_specs(
        &project.manifest.dependencies,
        workspace.as_ref(),
        &mut local_deps,
        Some(&mut manifest_protocols),
    )?;
    let dev_deps = apply_specs(
        &project.manifest.dev_dependencies,
        workspace.as_ref(),
        &mut local_dev_deps,
        Some(&mut manifest_protocols),
    )?;

    let root_deps = if let Some(ws) = workspace.as_ref() {
        collect_workspace_root_deps(ws, include_dev)?
    } else {
        build_project_manifest_root(&deps, &dev_deps, include_dev)
    };

    let mut root_protocols = BTreeMap::new();
    for name in root_deps.keys() {
        if let Some(proto) = manifest_protocols.get(name) {
            root_protocols.insert(name.clone(), proto.clone());
        } else {
            root_protocols.insert(name.clone(), RegistryProtocol::npm());
        }
    }

    let graph = resolve::resolve(
        config,
        &root_deps,
        &root_protocols,
        config.min_package_age_days,
        false,
        Some(&overrides),
    )
    .await?;

    let lockfile_path = workspace
        .as_ref()
        .map(|w| w.root.join("snpm-lock.yaml"))
        .unwrap_or_else(|| project.root.join("snpm-lock.yaml"));

    let mut current_versions = BTreeMap::new();

    if lockfile_path.is_file() {
        let existing = lockfile::read(&lockfile_path)?;

        for (name, dep) in existing.root.dependencies.iter() {
            current_versions.insert(name.clone(), dep.version.clone());
        }
    }

    let mut names = BTreeSet::new();
    for name in deps.keys() {
        names.insert(name.clone());
    }
    if include_dev {
        for name in dev_deps.keys() {
            names.insert(name.clone());
        }
    }

    let mut result = Vec::new();

    for name in names {
        let root_dep = match graph.root.dependencies.get(&name) {
            Some(dep) => dep,
            None => continue,
        };

        let wanted = root_dep.resolved.version.clone();
        let current = current_versions.get(&name).cloned();

        if let Some(ref cur) = current {
            if cur == &wanted {
                continue;
            }
        }

        result.push(OutdatedEntry {
            name,
            current,
            wanted,
        });
    }

    Ok(result)
}

fn parse_requested_with_protocol(
    specs: &[String],
) -> (BTreeMap<String, String>, BTreeMap<String, RegistryProtocol>) {
    let mut ranges = BTreeMap::new();
    let mut protocols = BTreeMap::new();

    for spec in specs {
        let parsed = parse_requested_spec(spec);
        ranges.insert(parsed.name.clone(), parsed.range.clone());

        if let Some(proto) = parsed.protocol.as_deref() {
            let protocol = match proto {
                "npm" => RegistryProtocol::npm(),
                "jsr" => RegistryProtocol::jsr(),
                other => RegistryProtocol::custom(other),
            };
            protocols.insert(parsed.name.clone(), protocol);
        }
    }

    (ranges, protocols)
}

fn parse_manifest_protocol(spec: &str) -> Option<(String, RegistryProtocol)> {
    if !(spec.starts_with("npm:") || spec.starts_with("jsr:")) {
        return None;
    }

    let parsed = parse_requested_spec(spec);

    let proto_name = parsed.protocol.as_deref()?;
    let protocol = match proto_name {
        "npm" => RegistryProtocol::npm(),
        "jsr" => RegistryProtocol::jsr(),
        _ => return None,
    };

    Some((parsed.range, protocol))
}

fn project_label(project: &Project) -> String {
    if let Some(name) = project.manifest.name.as_deref() {
        name.to_string()
    } else {
        project
            .root
            .file_name()
            .and_then(|os| os.to_str())
            .unwrap_or(".")
            .to_string()
    }
}
