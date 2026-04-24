use crate::patch::{get_patched_dependencies, parse_patch_key};
use crate::resolve::{PackageId, ResolutionGraph, ResolvedPackage};
use crate::store::PACKAGE_METADATA_FILE;
use crate::{Project, Result, SnpmConfig, SnpmError, Workspace, lifecycle};

use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use super::fs::{
    copy_dir, ensure_parent_dir, link_dir, package_node_modules, symlink_dir_entry,
    symlink_is_correct,
};

const VIRTUAL_DEPENDENCY_STATE_FILE: &str = ".snpm-dependencies.bin";
const VIRTUAL_DEPENDENCY_STATE_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct VirtualDependencyState {
    version: u32,
    dependency_hash: String,
    directory_seconds: u64,
    directory_nanoseconds: u32,
}

pub(crate) fn populate_shared_virtual_store_for_packages(
    config: &SnpmConfig,
    graph: &ResolutionGraph,
    store_paths: &BTreeMap<PackageId, PathBuf>,
    shared_package_ids: &BTreeSet<PackageId>,
) -> Result<Arc<BTreeMap<PackageId, PathBuf>>> {
    let shared_virtual_store_dir = config.virtual_store_dir();
    fs::create_dir_all(&shared_virtual_store_dir).map_err(|source| SnpmError::WriteFile {
        path: shared_virtual_store_dir.clone(),
        source,
    })?;

    let entry_hashes = global_virtual_store_entry_hashes(graph);
    let shared_paths = materialize_virtual_store(
        &shared_virtual_store_dir,
        store_paths,
        config,
        shared_package_ids,
        Some(&entry_hashes),
    )?;
    link_selected_virtual_dependencies(shared_paths.as_ref(), graph, shared_package_ids)?;

    Ok(shared_paths)
}

pub(super) fn populate_virtual_store(
    virtual_store_dir: &Path,
    graph: &ResolutionGraph,
    store_paths: &BTreeMap<PackageId, PathBuf>,
    config: &SnpmConfig,
    workspace: Option<&Workspace>,
    project: &Project,
) -> Result<Arc<BTreeMap<PackageId, PathBuf>>> {
    let locally_materialized_ids =
        local_global_virtual_store_package_ids(config, workspace, &[project], graph);
    log_locally_materialized_packages(&locally_materialized_ids);

    let shared_package_ids = shared_package_ids(graph, &locally_materialized_ids);
    let shared_paths = populate_shared_virtual_store_for_packages(
        config,
        graph,
        store_paths,
        &shared_package_ids,
    )?;
    let packages: Vec<_> = graph.packages.keys().collect();
    let results: Vec<Result<(PackageId, PathBuf)>> = packages
        .par_iter()
        .map(|id| -> Result<(PackageId, PathBuf)> {
            let package_location = virtual_package_location(virtual_store_dir, id);
            let marker_file = virtual_marker_path(virtual_store_dir, id);
            let should_materialize_locally = locally_materialized_ids.contains(*id);

            if should_materialize_locally {
                if marker_file.is_file() && virtual_package_ready(&package_location) {
                    return Ok(((*id).clone(), package_location));
                }
            } else {
                let shared_target =
                    shared_paths
                        .get(*id)
                        .ok_or_else(|| SnpmError::StoreMissing {
                            name: id.name.clone(),
                            version: id.version.clone(),
                        })?;

                if symlink_is_correct(&package_location, shared_target) {
                    return Ok(((*id).clone(), package_location));
                }
            }

            if marker_file.is_file() {
                fs::remove_file(&marker_file).ok();
            }

            let store_path = store_paths.get(id).ok_or_else(|| SnpmError::StoreMissing {
                name: id.name.clone(),
                version: id.version.clone(),
            })?;

            fs::remove_file(&package_location).ok();
            fs::remove_dir_all(&package_location).ok();

            ensure_parent_dir(&package_location)?;

            if should_materialize_locally {
                link_dir(config, store_path, &package_location)?;
                fs::write(&marker_file, []).map_err(|source| SnpmError::WriteFile {
                    path: marker_file,
                    source,
                })?;
            } else {
                let shared_target =
                    shared_paths
                        .get(*id)
                        .ok_or_else(|| SnpmError::StoreMissing {
                            name: id.name.clone(),
                            version: id.version.clone(),
                        })?;
                if symlink_dir_entry(shared_target, &package_location).is_err() {
                    link_dir(config, store_path, &package_location)?;
                    fs::write(&marker_file, []).map_err(|source| SnpmError::WriteFile {
                        path: marker_file,
                        source,
                    })?;
                }
            }

            Ok(((*id).clone(), package_location))
        })
        .collect();

    let mut map = BTreeMap::new();
    for result in results {
        let (id, path) = result?;
        map.insert(id, path);
    }

    Ok(Arc::new(map))
}

pub(crate) fn local_global_virtual_store_package_ids(
    config: &SnpmConfig,
    workspace: Option<&Workspace>,
    projects: &[&Project],
    graph: &ResolutionGraph,
) -> BTreeSet<PackageId> {
    let patched_package_ids = patched_package_ids(projects);
    let mut local = BTreeSet::new();

    for (id, package) in &graph.packages {
        if package_is_project_local(config, workspace, &patched_package_ids, id, package) {
            local.insert(id.clone());
        }
    }

    let mut changed = true;
    while changed {
        changed = false;
        for (id, package) in &graph.packages {
            if local.contains(id) {
                continue;
            }
            if package
                .dependencies
                .values()
                .any(|dep_id| local.contains(dep_id))
            {
                local.insert(id.clone());
                changed = true;
            }
        }
    }

    local
}

pub(crate) fn log_locally_materialized_packages(package_ids: &BTreeSet<PackageId>) {
    if package_ids.is_empty() {
        return;
    }

    let names = package_ids
        .iter()
        .map(|id| id.name.as_str())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join(", ");
    crate::console::verbose(&format!(
        "materializing {names} project-locally because they are not global-store safe"
    ));
}

fn package_disables_global_virtual_store(
    config: &SnpmConfig,
    workspace: Option<&Workspace>,
    package_name: &str,
) -> bool {
    if let Some(workspace) = workspace
        && let Some(packages) = &workspace.config.disable_global_virtual_store_for_packages
    {
        return packages
            .iter()
            .any(|candidate| candidate.as_str() == package_name);
    }

    config
        .disable_global_virtual_store_for_packages
        .contains(package_name)
}

fn package_is_project_local(
    config: &SnpmConfig,
    workspace: Option<&Workspace>,
    patched_package_ids: &BTreeSet<PackageId>,
    id: &PackageId,
    package: &ResolvedPackage,
) -> bool {
    package_disables_global_virtual_store(config, workspace, &id.name)
        || patched_package_ids.contains(id)
        || lifecycle::is_dep_script_allowed(config, workspace, &id.name)
        || package_has_project_local_source(package)
}

fn package_has_project_local_source(package: &ResolvedPackage) -> bool {
    let Some(path) = package.tarball.strip_prefix("file://") else {
        return false;
    };

    Path::new(path).is_dir()
}

fn patched_package_ids(projects: &[&Project]) -> BTreeSet<PackageId> {
    projects
        .iter()
        .flat_map(|project| get_patched_dependencies(project).into_keys())
        .filter_map(|key| parse_patch_key(&key))
        .map(|(name, version)| PackageId { name, version })
        .collect()
}

fn shared_package_ids(
    graph: &ResolutionGraph,
    locally_materialized_ids: &BTreeSet<PackageId>,
) -> BTreeSet<PackageId> {
    graph
        .packages
        .keys()
        .filter(|id| !locally_materialized_ids.contains(*id))
        .cloned()
        .collect()
}

fn materialize_virtual_store(
    virtual_store_dir: &Path,
    store_paths: &BTreeMap<PackageId, PathBuf>,
    config: &SnpmConfig,
    package_ids: &BTreeSet<PackageId>,
    entry_hashes: Option<&BTreeMap<PackageId, String>>,
) -> Result<Arc<BTreeMap<PackageId, PathBuf>>> {
    let packages: Vec<_> = package_ids.iter().collect();
    let results: Vec<Result<(PackageId, PathBuf)>> = packages
        .par_iter()
        .map(|id| -> Result<(PackageId, PathBuf)> {
            let package_location =
                hashed_virtual_package_location(virtual_store_dir, id, entry_hashes);
            let marker_file = hashed_virtual_marker_path(virtual_store_dir, id, entry_hashes);

            let store_path = store_paths.get(id).ok_or_else(|| SnpmError::StoreMissing {
                name: id.name.clone(),
                version: id.version.clone(),
            })?;

            if marker_file.is_file() && virtual_package_ready(&package_location) {
                return Ok(((*id).clone(), package_location));
            }

            if marker_file.is_file() {
                fs::remove_file(&marker_file).ok();
            }

            fs::remove_file(&package_location).ok();
            fs::remove_dir_all(&package_location).ok();

            ensure_parent_dir(&package_location)?;
            link_dir(config, store_path, &package_location)?;
            fs::write(&marker_file, []).map_err(|source| SnpmError::WriteFile {
                path: marker_file,
                source,
            })?;

            Ok(((*id).clone(), package_location))
        })
        .collect();

    let mut map = BTreeMap::new();
    for result in results {
        let (id, path) = result?;
        map.insert(id, path);
    }

    Ok(Arc::new(map))
}

pub(crate) fn link_virtual_dependencies(
    virtual_store_paths: &BTreeMap<PackageId, PathBuf>,
    graph: &ResolutionGraph,
) -> Result<()> {
    let package_ids = graph.packages.keys().cloned().collect();
    link_selected_virtual_dependencies(virtual_store_paths, graph, &package_ids)
}

fn link_selected_virtual_dependencies(
    virtual_store_paths: &BTreeMap<PackageId, PathBuf>,
    graph: &ResolutionGraph,
    package_ids: &BTreeSet<PackageId>,
) -> Result<()> {
    let packages: Vec<_> = package_ids
        .iter()
        .map(|id| {
            graph
                .packages
                .get_key_value(id)
                .ok_or_else(|| SnpmError::GraphMissing {
                    name: id.name.clone(),
                    version: id.version.clone(),
                })
        })
        .collect::<Result<Vec<_>>>()?;

    packages
        .par_iter()
        .try_for_each(|(id, package)| -> Result<()> {
            let package_location =
                virtual_store_paths
                    .get(id)
                    .ok_or_else(|| SnpmError::GraphMissing {
                        name: id.name.clone(),
                        version: id.version.clone(),
                    })?;
            let package_node_modules = package_node_modules(package_location, &id.name)
                .ok_or_else(|| SnpmError::GraphMissing {
                    name: id.name.clone(),
                    version: id.version.clone(),
                })?;

            if package.dependencies.is_empty() {
                remove_dependency_state(package_location, &id.name);
                return Ok(());
            }

            let dependency_hash = dependency_state_hash(package, virtual_store_paths)?;
            if dependency_state_matches(package_location, &id.name, &dependency_hash) {
                return Ok(());
            }

            for (dep_name, dep_id) in &package.dependencies {
                let dep_target =
                    virtual_store_paths
                        .get(dep_id)
                        .ok_or_else(|| SnpmError::GraphMissing {
                            name: dep_id.name.clone(),
                            version: dep_id.version.clone(),
                        })?;
                let dep_link = package_node_modules.join(dep_name);

                if symlink_is_correct(&dep_link, dep_target) {
                    continue;
                }

                std::fs::remove_file(&dep_link).ok();
                std::fs::remove_dir_all(&dep_link).ok();

                ensure_parent_dir(&dep_link)?;
                symlink_dir_entry(dep_target, &dep_link)
                    .or_else(|_| copy_dir(dep_target, &dep_link))?;
            }

            write_dependency_state(package_location, &id.name, &dependency_hash)?;

            Ok(())
        })
}

fn virtual_id_dir(virtual_store_dir: &Path, id: &PackageId) -> PathBuf {
    let safe_name = id.name.replace('/', "+");
    virtual_store_dir.join(format!("{}@{}", safe_name, id.version))
}

fn hashed_virtual_id_dir(
    virtual_store_dir: &Path,
    id: &PackageId,
    entry_hashes: Option<&BTreeMap<PackageId, String>>,
) -> PathBuf {
    let safe_name = id.name.replace('/', "+");
    match entry_hashes.and_then(|hashes| hashes.get(id)) {
        Some(hash) => virtual_store_dir.join(format!("{}@{}-{}", safe_name, id.version, hash)),
        None => virtual_id_dir(virtual_store_dir, id),
    }
}

fn virtual_package_location(virtual_store_dir: &Path, id: &PackageId) -> PathBuf {
    virtual_id_dir(virtual_store_dir, id)
        .join("node_modules")
        .join(&id.name)
}

fn hashed_virtual_package_location(
    virtual_store_dir: &Path,
    id: &PackageId,
    entry_hashes: Option<&BTreeMap<PackageId, String>>,
) -> PathBuf {
    hashed_virtual_id_dir(virtual_store_dir, id, entry_hashes)
        .join("node_modules")
        .join(&id.name)
}

fn virtual_marker_path(virtual_store_dir: &Path, id: &PackageId) -> PathBuf {
    virtual_id_dir(virtual_store_dir, id).join(".snpm_linked")
}

fn hashed_virtual_marker_path(
    virtual_store_dir: &Path,
    id: &PackageId,
    entry_hashes: Option<&BTreeMap<PackageId, String>>,
) -> PathBuf {
    hashed_virtual_id_dir(virtual_store_dir, id, entry_hashes).join(".snpm_linked")
}

fn global_virtual_store_entry_hashes(graph: &ResolutionGraph) -> BTreeMap<PackageId, String> {
    let mut hashes = BTreeMap::new();
    let mut visiting = BTreeSet::new();

    for id in graph.packages.keys() {
        compute_global_virtual_store_entry_hash(id, graph, &mut hashes, &mut visiting);
    }

    hashes
}

fn compute_global_virtual_store_entry_hash(
    id: &PackageId,
    graph: &ResolutionGraph,
    hashes: &mut BTreeMap<PackageId, String>,
    visiting: &mut BTreeSet<PackageId>,
) -> String {
    if let Some(hash) = hashes.get(id) {
        return hash.clone();
    }

    let mut hasher = Sha256::new();
    hasher.update(b"snpm-global-virtual-store-v1");
    hash_package_id(&mut hasher, id);

    if !visiting.insert(id.clone()) {
        hasher.update(b"cycle");
        let digest = hasher.finalize();
        return short_hash(&digest);
    }

    if let Some(package) = graph.packages.get(id) {
        hasher.update(package.tarball.as_bytes());
        hasher.update([0]);
        if let Some(integrity) = &package.integrity {
            hasher.update(integrity.as_bytes());
        }
        hasher.update([0]);

        for (dep_name, dep_id) in &package.dependencies {
            hasher.update(dep_name.as_bytes());
            hasher.update([0]);
            hash_package_id(&mut hasher, dep_id);
            let dep_hash = compute_global_virtual_store_entry_hash(dep_id, graph, hashes, visiting);
            hasher.update(dep_hash.as_bytes());
            hasher.update([0]);
        }
    }

    visiting.remove(id);
    let digest = hasher.finalize();
    let hash = short_hash(&digest);
    hashes.insert(id.clone(), hash.clone());
    hash
}

fn hash_package_id(hasher: &mut Sha256, id: &PackageId) {
    hasher.update(id.name.as_bytes());
    hasher.update([0]);
    hasher.update(id.version.as_bytes());
    hasher.update([0]);
}

fn short_hash(bytes: &[u8]) -> String {
    hex::encode(bytes)[..16].to_string()
}

fn virtual_package_ready(package_location: &Path) -> bool {
    let is_real_dir = package_location
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink());

    is_real_dir
        && (package_location.join(PACKAGE_METADATA_FILE).is_file()
            || fs::read_dir(package_location)
                .ok()
                .and_then(|mut entries| entries.next())
                .is_some())
}

fn dependency_state_hash(
    package: &crate::resolve::ResolvedPackage,
    virtual_store_paths: &BTreeMap<PackageId, PathBuf>,
) -> Result<String> {
    let mut hasher = Sha256::new();

    for (dep_name, dep_id) in &package.dependencies {
        let dep_target =
            virtual_store_paths
                .get(dep_id)
                .ok_or_else(|| SnpmError::GraphMissing {
                    name: dep_id.name.clone(),
                    version: dep_id.version.clone(),
                })?;

        hasher.update(dep_name.as_bytes());
        hasher.update([0]);
        hasher.update(dep_id.name.as_bytes());
        hasher.update([0]);
        hasher.update(dep_id.version.as_bytes());
        hasher.update([0]);
        hasher.update(dep_target.as_os_str().as_encoded_bytes());
        hasher.update([0]);
    }

    Ok(hex::encode(hasher.finalize()))
}

fn dependency_state_matches(
    package_location: &Path,
    package_name: &str,
    expected_hash: &str,
) -> bool {
    let dependency_dir = match package_node_modules(package_location, package_name) {
        Some(path) => path,
        None => return false,
    };
    let state_path = match dependency_state_path(package_location, package_name) {
        Some(path) => path,
        None => return false,
    };
    let Some(state) = read_dependency_state_lossy(&state_path) else {
        return false;
    };

    if state.version != VIRTUAL_DEPENDENCY_STATE_VERSION || state.dependency_hash != expected_hash {
        return false;
    }

    let Ok(metadata) = fs::metadata(&dependency_dir) else {
        return false;
    };
    let Ok(modified) = metadata.modified() else {
        return false;
    };
    let Ok(duration) = modified.duration_since(UNIX_EPOCH) else {
        return false;
    };

    duration.as_secs() == state.directory_seconds
        && duration.subsec_nanos() == state.directory_nanoseconds
}

fn write_dependency_state(
    package_location: &Path,
    package_name: &str,
    dependency_hash: &str,
) -> Result<()> {
    let dependency_dir = package_node_modules(package_location, package_name).ok_or_else(|| {
        SnpmError::Internal {
            reason: format!(
                "virtual package path missing dependency container: {}",
                package_location.display()
            ),
        }
    })?;
    let state_path = dependency_state_path(package_location, package_name).ok_or_else(|| {
        SnpmError::Internal {
            reason: format!(
                "virtual package path missing state directory: {}",
                package_location.display()
            ),
        }
    })?;
    let metadata = fs::metadata(&dependency_dir).map_err(|source| SnpmError::ReadFile {
        path: dependency_dir.clone(),
        source,
    })?;
    let modified = metadata.modified().map_err(|source| SnpmError::ReadFile {
        path: dependency_dir.clone(),
        source,
    })?;
    let duration = modified
        .duration_since(UNIX_EPOCH)
        .map_err(|source| SnpmError::Io {
            path: dependency_dir,
            source: std::io::Error::other(source),
        })?;
    let state = VirtualDependencyState {
        version: VIRTUAL_DEPENDENCY_STATE_VERSION,
        dependency_hash: dependency_hash.to_string(),
        directory_seconds: duration.as_secs(),
        directory_nanoseconds: duration.subsec_nanos(),
    };
    let data = bincode::serialize(&state).map_err(|source| SnpmError::SerializeJson {
        path: state_path.clone(),
        reason: source.to_string(),
    })?;

    fs::write(&state_path, data).map_err(|source| SnpmError::WriteFile {
        path: state_path,
        source,
    })
}

fn read_dependency_state_lossy(path: &Path) -> Option<VirtualDependencyState> {
    let bytes = fs::read(path).ok()?;
    bincode::deserialize(&bytes).ok()
}

fn remove_dependency_state(package_location: &Path, package_name: &str) {
    if let Some(path) = dependency_state_path(package_location, package_name) {
        fs::remove_file(path).ok();
    }
}

fn dependency_state_path(package_location: &Path, package_name: &str) -> Option<PathBuf> {
    package_node_modules(package_location, package_name)?
        .parent()
        .map(|parent| parent.join(VIRTUAL_DEPENDENCY_STATE_FILE))
}

#[cfg(test)]
mod tests {
    use super::{
        dependency_state_hash, dependency_state_matches, dependency_state_path,
        link_virtual_dependencies, local_global_virtual_store_package_ids, populate_virtual_store,
    };
    use crate::Project;
    use crate::Workspace;
    use crate::config::{AuthScheme, HoistingMode, LinkBackend, SnpmConfig};
    use crate::linker::fs::package_node_modules;
    use crate::project::{Manifest, ManifestSnpm};
    use crate::resolve::{
        PackageId, ResolutionGraph, ResolutionRoot, ResolvedPackage, RootDependency,
    };
    use crate::workspace::types::WorkspaceConfig;

    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn make_config(data_dir: PathBuf) -> SnpmConfig {
        SnpmConfig {
            cache_dir: data_dir.join("cache"),
            data_dir,
            allow_scripts: BTreeSet::new(),
            disable_global_virtual_store_for_packages: BTreeSet::new(),
            min_package_age_days: None,
            min_package_cache_age_days: None,
            default_registry: "https://registry.npmjs.org".to_string(),
            scoped_registries: BTreeMap::new(),
            registry_auth: BTreeMap::new(),
            default_registry_auth_token: None,
            default_registry_auth_scheme: AuthScheme::Bearer,
            registry_auth_schemes: BTreeMap::new(),
            hoisting: HoistingMode::SingleVersion,
            link_backend: LinkBackend::Auto,
            strict_peers: false,
            frozen_lockfile_default: false,
            always_auth: false,
            registry_concurrency: 64,
            verbose: false,
            log_file: None,
        }
    }

    fn make_graph(id: &PackageId) -> ResolutionGraph {
        let pkg = ResolvedPackage {
            id: id.clone(),
            tarball: String::new(),
            integrity: None,
            dependencies: BTreeMap::new(),
            peer_dependencies: BTreeMap::new(),
            bundled_dependencies: None,
            has_bin: false,
            bin: None,
        };

        ResolutionGraph {
            root: ResolutionRoot {
                dependencies: BTreeMap::new(),
            },
            packages: BTreeMap::from([(id.clone(), pkg)]),
        }
    }

    fn make_graph_with_root_dependency(id: &PackageId) -> ResolutionGraph {
        let mut graph = make_graph(id);
        graph.root.dependencies.insert(
            id.name.clone(),
            RootDependency {
                requested: id.version.clone(),
                resolved: id.clone(),
            },
        );
        graph
    }

    fn make_graph_with_transitive_dep(
        root_id: &PackageId,
        child_id: &PackageId,
    ) -> ResolutionGraph {
        let root_pkg = ResolvedPackage {
            id: root_id.clone(),
            tarball: String::new(),
            integrity: None,
            dependencies: BTreeMap::from([("child".to_string(), child_id.clone())]),
            peer_dependencies: BTreeMap::new(),
            bundled_dependencies: None,
            has_bin: false,
            bin: None,
        };
        let child_pkg = ResolvedPackage {
            id: child_id.clone(),
            tarball: String::new(),
            integrity: None,
            dependencies: BTreeMap::new(),
            peer_dependencies: BTreeMap::new(),
            bundled_dependencies: None,
            has_bin: false,
            bin: None,
        };

        ResolutionGraph {
            root: ResolutionRoot {
                dependencies: BTreeMap::new(),
            },
            packages: BTreeMap::from([(root_id.clone(), root_pkg), (child_id.clone(), child_pkg)]),
        }
    }

    fn make_workspace(only_built_dependencies: Vec<String>) -> Workspace {
        Workspace {
            root: PathBuf::from("/tmp/workspace"),
            projects: Vec::new(),
            config: WorkspaceConfig {
                packages: Vec::new(),
                catalog: BTreeMap::new(),
                catalogs: BTreeMap::new(),
                only_built_dependencies,
                ignored_built_dependencies: Vec::new(),
                disable_global_virtual_store_for_packages: None,
                hoisting: None,
            },
        }
    }

    fn make_project(root: PathBuf) -> Project {
        Project {
            manifest_path: root.join("package.json"),
            root,
            manifest: Manifest {
                name: Some("app".to_string()),
                version: Some("1.0.0".to_string()),
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
        }
    }

    fn write_package(path: &std::path::Path, name: &str, version: &str) {
        fs::create_dir_all(path).unwrap();
        fs::write(
            path.join("package.json"),
            format!(r#"{{"name":"{name}","version":"{version}"}}"#),
        )
        .unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn populate_virtual_store_reuses_shared_packages_across_projects() {
        let dir = tempdir().unwrap();
        let config = make_config(dir.path().join("data"));

        let id = PackageId {
            name: "left-pad".to_string(),
            version: "1.3.0".to_string(),
        };
        let graph = make_graph(&id);
        let store_path = dir.path().join("store/left-pad");

        fs::create_dir_all(&store_path).unwrap();
        fs::write(
            store_path.join("package.json"),
            r#"{"name":"left-pad","version":"1.3.0"}"#,
        )
        .unwrap();

        let store_paths = BTreeMap::from([(id.clone(), store_path)]);
        let project_one_store = dir.path().join("project-one/.snpm");
        let project_two_store = dir.path().join("project-two/.snpm");

        let project_one = make_project(dir.path().join("project-one"));
        let project_one_paths = populate_virtual_store(
            &project_one_store,
            &graph,
            &store_paths,
            &config,
            None,
            &project_one,
        )
        .unwrap();

        let shared_target = project_one_paths.get(&id).unwrap();
        fs::write(shared_target.join("shared.txt"), "reused").unwrap();

        let project_two = make_project(dir.path().join("project-two"));
        let project_two_paths = populate_virtual_store(
            &project_two_store,
            &graph,
            &store_paths,
            &config,
            None,
            &project_two,
        )
        .unwrap();
        let local_target = project_two_paths.get(&id).unwrap();

        assert!(local_target.join("shared.txt").is_file());
        assert!(
            local_target
                .symlink_metadata()
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }

    #[test]
    fn populate_virtual_store_keeps_script_allowed_packages_local() {
        let dir = tempdir().unwrap();
        let config = make_config(dir.path().join("data"));

        let id = PackageId {
            name: "esbuild".to_string(),
            version: "1.0.0".to_string(),
        };
        let graph = make_graph(&id);
        let store_path = dir.path().join("store/esbuild");

        fs::create_dir_all(&store_path).unwrap();
        fs::write(
            store_path.join("package.json"),
            r#"{"name":"esbuild","version":"1.0.0","scripts":{"postinstall":"echo build"}}"#,
        )
        .unwrap();

        let store_paths = BTreeMap::from([(id.clone(), store_path)]);
        let workspace = make_workspace(vec!["esbuild".to_string()]);
        let project_store = dir.path().join("project/.snpm");
        let project = make_project(dir.path().join("project"));

        let paths = populate_virtual_store(
            &project_store,
            &graph,
            &store_paths,
            &config,
            Some(&workspace),
            &project,
        )
        .unwrap();
        let local_target = paths.get(&id).unwrap();

        assert!(local_target.is_dir());
        assert!(
            !local_target
                .symlink_metadata()
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }

    #[test]
    fn populate_virtual_store_keeps_configured_root_packages_local() {
        let dir = tempdir().unwrap();
        let mut config = make_config(dir.path().join("data"));
        config
            .disable_global_virtual_store_for_packages
            .insert("vite".to_string());

        let id = PackageId {
            name: "vite".to_string(),
            version: "5.0.0".to_string(),
        };
        let graph = make_graph_with_root_dependency(&id);
        let store_path = dir.path().join("store/vite");

        fs::create_dir_all(&store_path).unwrap();
        fs::write(
            store_path.join("package.json"),
            r#"{"name":"vite","version":"5.0.0"}"#,
        )
        .unwrap();

        let store_paths = BTreeMap::from([(id.clone(), store_path)]);
        let project_store = dir.path().join("project/.snpm");
        let project = make_project(dir.path().join("project"));

        let paths = populate_virtual_store(
            &project_store,
            &graph,
            &store_paths,
            &config,
            None,
            &project,
        )
        .unwrap();
        let local_target = paths.get(&id).unwrap();

        assert!(local_target.is_dir());
        assert!(
            !local_target
                .symlink_metadata()
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }

    #[test]
    fn populate_virtual_store_propagates_project_local_sources_to_dependents() {
        let dir = tempdir().unwrap();
        let config = make_config(dir.path().join("data"));

        let root_id = PackageId {
            name: "parent".to_string(),
            version: "1.0.0".to_string(),
        };
        let child_id = PackageId {
            name: "local-child".to_string(),
            version: "1.0.0".to_string(),
        };
        let mut graph = make_graph_with_transitive_dep(&root_id, &child_id);
        let local_source = dir.path().join("local-child-source");
        fs::create_dir_all(&local_source).unwrap();
        graph.packages.get_mut(&child_id).unwrap().tarball =
            format!("file://{}", local_source.display());

        let root_store = dir.path().join("store/parent");
        let child_store = dir.path().join("store/local-child");
        write_package(&root_store, "parent", "1.0.0");
        write_package(&child_store, "local-child", "1.0.0");

        let store_paths = BTreeMap::from([
            (root_id.clone(), root_store),
            (child_id.clone(), child_store),
        ]);
        let project_store = dir.path().join("project/.snpm");
        let project = make_project(dir.path().join("project"));

        let paths = populate_virtual_store(
            &project_store,
            &graph,
            &store_paths,
            &config,
            None,
            &project,
        )
        .unwrap();

        for id in [&root_id, &child_id] {
            let target = paths.get(id).unwrap();
            assert!(target.is_dir());
            assert!(!target.symlink_metadata().unwrap().file_type().is_symlink());
        }
    }

    #[test]
    fn local_virtual_store_package_ids_include_patched_packages_and_dependents() {
        let dir = tempdir().unwrap();
        let config = make_config(dir.path().join("data"));

        let root_id = PackageId {
            name: "parent".to_string(),
            version: "1.0.0".to_string(),
        };
        let child_id = PackageId {
            name: "patched-child".to_string(),
            version: "1.0.0".to_string(),
        };
        let graph = make_graph_with_transitive_dep(&root_id, &child_id);
        let mut project = make_project(dir.path().join("project"));
        project.manifest.snpm = Some(ManifestSnpm {
            overrides: BTreeMap::new(),
            patched_dependencies: Some(BTreeMap::from([(
                "patched-child@1.0.0".to_string(),
                "patches/patched-child@1.0.0.patch".to_string(),
            )])),
            publish: None,
        });

        let local_ids = local_global_virtual_store_package_ids(&config, None, &[&project], &graph);

        assert!(local_ids.contains(&child_id));
        assert!(local_ids.contains(&root_id));
    }

    #[cfg(unix)]
    #[test]
    fn populate_virtual_store_hashes_shared_entries_by_dependency_closure() {
        let dir = tempdir().unwrap();
        let config = make_config(dir.path().join("data"));

        let root_id = PackageId {
            name: "shared-parent".to_string(),
            version: "1.0.0".to_string(),
        };
        let child_one_id = PackageId {
            name: "child".to_string(),
            version: "1.0.0".to_string(),
        };
        let child_two_id = PackageId {
            name: "child".to_string(),
            version: "2.0.0".to_string(),
        };

        let graph_one = make_graph_with_transitive_dep(&root_id, &child_one_id);
        let graph_two = make_graph_with_transitive_dep(&root_id, &child_two_id);

        let root_store = dir.path().join("store/shared-parent");
        let child_one_store = dir.path().join("store/child-one");
        let child_two_store = dir.path().join("store/child-two");
        write_package(&root_store, "shared-parent", "1.0.0");
        write_package(&child_one_store, "child", "1.0.0");
        write_package(&child_two_store, "child", "2.0.0");

        let store_paths_one = BTreeMap::from([
            (root_id.clone(), root_store.clone()),
            (child_one_id.clone(), child_one_store),
        ]);
        let store_paths_two = BTreeMap::from([
            (root_id.clone(), root_store),
            (child_two_id.clone(), child_two_store),
        ]);

        let project_one = make_project(dir.path().join("project-one"));
        let project_two = make_project(dir.path().join("project-two"));
        let paths_one = populate_virtual_store(
            &dir.path().join("project-one/.snpm"),
            &graph_one,
            &store_paths_one,
            &config,
            None,
            &project_one,
        )
        .unwrap();
        let paths_two = populate_virtual_store(
            &dir.path().join("project-two/.snpm"),
            &graph_two,
            &store_paths_two,
            &config,
            None,
            &project_two,
        )
        .unwrap();

        let target_one = fs::read_link(paths_one.get(&root_id).unwrap()).unwrap();
        let target_two = fs::read_link(paths_two.get(&root_id).unwrap()).unwrap();

        assert_ne!(target_one, target_two);
    }

    #[cfg(unix)]
    #[test]
    fn link_virtual_dependencies_reuses_persisted_dependency_state() {
        let dir = tempdir().unwrap();
        let root_id = PackageId {
            name: "dep".to_string(),
            version: "1.0.0".to_string(),
        };
        let child_id = PackageId {
            name: "child".to_string(),
            version: "1.0.0".to_string(),
        };
        let graph = make_graph_with_transitive_dep(&root_id, &child_id);
        let root_location = dir.path().join(".snpm/dep@1.0.0/node_modules/dep");
        let child_location = dir.path().join(".snpm/child@1.0.0/node_modules/child");

        fs::create_dir_all(root_location.join("node_modules")).unwrap();
        fs::create_dir_all(&child_location).unwrap();
        fs::write(root_location.join("package.json"), "{}").unwrap();
        fs::write(child_location.join("package.json"), "{}").unwrap();

        let virtual_store_paths = BTreeMap::from([
            (root_id.clone(), root_location.clone()),
            (child_id.clone(), child_location.clone()),
        ]);

        link_virtual_dependencies(&virtual_store_paths, &graph).unwrap();

        let dependency_hash =
            dependency_state_hash(graph.packages.get(&root_id).unwrap(), &virtual_store_paths)
                .unwrap();
        assert!(
            dependency_state_path(&root_location, &root_id.name)
                .unwrap()
                .is_file()
        );
        assert!(dependency_state_matches(
            &root_location,
            &root_id.name,
            &dependency_hash
        ));

        fs::remove_file(
            package_node_modules(&root_location, &root_id.name)
                .unwrap()
                .join("child"),
        )
        .unwrap();

        assert!(!dependency_state_matches(
            &root_location,
            &root_id.name,
            &dependency_hash
        ));
    }
}
