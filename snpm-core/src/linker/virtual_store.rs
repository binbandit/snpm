use crate::resolve::{PackageId, ResolutionGraph};
use crate::store::PACKAGE_METADATA_FILE;
use crate::{Result, SnpmConfig, SnpmError, Workspace, lifecycle};

use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
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

pub(crate) fn populate_shared_virtual_store(
    config: &SnpmConfig,
    graph: &ResolutionGraph,
    store_paths: &BTreeMap<PackageId, PathBuf>,
) -> Result<Arc<BTreeMap<PackageId, PathBuf>>> {
    let shared_virtual_store_dir = config.virtual_store_dir();
    fs::create_dir_all(&shared_virtual_store_dir).map_err(|source| SnpmError::WriteFile {
        path: shared_virtual_store_dir.clone(),
        source,
    })?;

    let shared_paths =
        materialize_virtual_store(&shared_virtual_store_dir, graph, store_paths, config)?;
    link_virtual_dependencies(shared_paths.as_ref(), graph)?;

    Ok(shared_paths)
}

pub(super) fn populate_virtual_store(
    virtual_store_dir: &Path,
    graph: &ResolutionGraph,
    store_paths: &BTreeMap<PackageId, PathBuf>,
    config: &SnpmConfig,
    workspace: Option<&Workspace>,
) -> Result<Arc<BTreeMap<PackageId, PathBuf>>> {
    let shared_paths = populate_shared_virtual_store(config, graph, store_paths)?;
    let packages: Vec<_> = graph.packages.keys().collect();
    let results: Vec<Result<(PackageId, PathBuf)>> = packages
        .par_iter()
        .map(|id| -> Result<(PackageId, PathBuf)> {
            let package_location = virtual_package_location(virtual_store_dir, id);
            let marker_file = virtual_marker_path(virtual_store_dir, id);
            let shared_target = shared_paths
                .get(*id)
                .ok_or_else(|| SnpmError::StoreMissing {
                    name: id.name.clone(),
                    version: id.version.clone(),
                })?;
            let scripts_allowed = lifecycle::is_dep_script_allowed(config, workspace, &id.name);

            if !scripts_allowed && symlink_is_correct(&package_location, shared_target) {
                return Ok(((*id).clone(), package_location));
            }

            if scripts_allowed {
                if marker_file.is_file() && virtual_package_ready(&package_location) {
                    return Ok(((*id).clone(), package_location));
                }
            } else if marker_file.is_file() && virtual_package_ready(&package_location) {
                return Ok(((*id).clone(), package_location));
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

            if scripts_allowed || symlink_dir_entry(shared_target, &package_location).is_err() {
                link_dir(config, store_path, &package_location)?;
                fs::write(&marker_file, []).map_err(|source| SnpmError::WriteFile {
                    path: marker_file,
                    source,
                })?;
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

fn materialize_virtual_store(
    virtual_store_dir: &Path,
    graph: &ResolutionGraph,
    store_paths: &BTreeMap<PackageId, PathBuf>,
    config: &SnpmConfig,
) -> Result<Arc<BTreeMap<PackageId, PathBuf>>> {
    let packages: Vec<_> = graph.packages.iter().collect();
    let results: Vec<Result<(PackageId, PathBuf)>> = packages
        .par_iter()
        .map(|(id, _package)| -> Result<(PackageId, PathBuf)> {
            let package_location = virtual_package_location(virtual_store_dir, id);
            let marker_file = virtual_marker_path(virtual_store_dir, id);

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
    let packages: Vec<_> = graph.packages.iter().collect();

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

fn virtual_package_location(virtual_store_dir: &Path, id: &PackageId) -> PathBuf {
    virtual_id_dir(virtual_store_dir, id)
        .join("node_modules")
        .join(&id.name)
}

fn virtual_marker_path(virtual_store_dir: &Path, id: &PackageId) -> PathBuf {
    virtual_id_dir(virtual_store_dir, id).join(".snpm_linked")
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
        link_virtual_dependencies, populate_virtual_store,
    };
    use crate::Workspace;
    use crate::config::{AuthScheme, HoistingMode, LinkBackend, SnpmConfig};
    use crate::linker::fs::package_node_modules;
    use crate::resolve::{PackageId, ResolutionGraph, ResolutionRoot, ResolvedPackage};
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
                hoisting: None,
            },
        }
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

        populate_virtual_store(&project_one_store, &graph, &store_paths, &config, None).unwrap();

        let shared_target = config
            .virtual_store_dir()
            .join("left-pad@1.3.0")
            .join("node_modules")
            .join("left-pad");
        fs::write(shared_target.join("shared.txt"), "reused").unwrap();

        let project_two_paths =
            populate_virtual_store(&project_two_store, &graph, &store_paths, &config, None)
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

        let paths = populate_virtual_store(
            &project_store,
            &graph,
            &store_paths,
            &config,
            Some(&workspace),
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
