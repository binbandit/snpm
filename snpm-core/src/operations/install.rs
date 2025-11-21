use crate::lockfile;
use crate::resolve::ResolutionGraph;
use crate::{Project, Result, SnpmConfig, linker, resolve, store};
use futures::future::join_all;
use reqwest::Client;
use std::collections::BTreeMap;
#[derive(Debug, Clone)]
pub struct InstallOptions {
    pub requested: Vec<String>,
    pub dev: bool,
    pub include_dev: bool,
}

pub async fn install(
    config: &SnpmConfig,
    project: &Project,
    options: InstallOptions,
) -> Result<()> {
    let additions = parse_requested(&options.requested);

    let mut manifest_root = project.manifest.dependencies.clone();

    if options.include_dev {
        for (name, range) in project.manifest.dev_dependencies.iter() {
            manifest_root.entry(name.clone()).or_insert(range.clone());
        }
    }

    let mut root_deps = manifest_root.clone();

    for (name, range) in additions.iter() {
        root_deps.insert(name.clone(), range.clone());
    }

    let lockfile_path = project.root.join("snpm-lock.yaml");

    let graph = if additions.is_empty() && lockfile_path.is_file() {
        let existing = lockfile::read(&lockfile_path)?;
        let mut lock_requested = BTreeMap::new();

        for (name, dep) in existing.root.dependencies.iter() {
            lock_requested.insert(name.clone(), dep.requested.clone());
        }

        if lock_requested == manifest_root {
            lockfile::to_graph(&existing)
        } else {
            let graph = resolve::resolve(&root_deps).await?;
            lockfile::write(&lockfile_path, &graph)?;
            graph
        }
    } else {
        let graph = resolve::resolve(&root_deps).await?;
        lockfile::write(&lockfile_path, &graph)?;
        graph
    };

    let store_paths = materialize_store(config, &graph).await?;
    write_manifest(project, &graph, &additions, options.dev)?;
    linker::link(project, &graph, &store_paths, options.include_dev)?;

    Ok(())
}

pub async fn remove(config: &SnpmConfig, project: &mut Project, specs: Vec<String>) -> Result<()> {
    if specs.is_empty() {
        return Ok(());
    }

    let mut manifest = project.manifest.clone();

    for spec in specs {
        let (name, _) = parse_spec(&spec);
        manifest.dependencies.remove(&name);
        manifest.dev_dependencies.remove(&name);
    }

    project.write_manifest(&manifest)?;
    project.manifest = manifest;

    let options = InstallOptions {
        requested: Vec::new(),
        dev: false,
        include_dev: true,
    };

    install(config, project, options).await
}

fn parse_requested(specs: &[String]) -> BTreeMap<String, String> {
    let mut result = BTreeMap::new();

    for spec in specs {
        let (name, range) = parse_spec(spec);
        result.insert(name, range);
    }

    result
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
) -> Result<BTreeMap<crate::resolve::PackageId, std::path::PathBuf>> {
    let client = Client::new();
    let mut futures = Vec::new();

    for package in graph.packages.values() {
        let config = config.clone();
        let client = client.clone();

        let future = async move {
            let path = store::ensure_package(&config, &package, &client).await?;
            let id = package.id.clone();
            Ok::<(crate::resolve::PackageId, std::path::PathBuf), crate::SnpmError>((id, path))
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
    project: &Project,
    graph: &ResolutionGraph,
    additions: &BTreeMap<String, String>,
    dev: bool,
) -> Result<()> {
    if additions.is_empty() {
        return Ok(());
    }

    let mut new_dependencies = project.manifest.dependencies.clone();
    let mut new_dev_dependencies = project.manifest.dev_dependencies.clone();

    for (name, dep) in graph.root.dependencies.iter() {
        if additions.contains_key(name) {
            let range = format!("^{}", dep.resolved.version);
            if dev {
                new_dev_dependencies.insert(name.clone(), range);
            } else {
                new_dependencies.insert(name.clone(), range);
            }
        }
    }

    let mut manifest = project.manifest.clone();
    manifest.dependencies = new_dependencies;
    manifest.dev_dependencies = new_dev_dependencies;

    project.write_manifest(&manifest)
}
