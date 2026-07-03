use super::super::keys::{package_key, split_dep_key};
use super::super::types::{LockPackage, LockRoot, LockRootDependency, Lockfile};
use crate::protocols::encode_package_name;
use crate::registry::{BundledDependencies, PeerDependencyMeta};
use crate::{Result, SnpmConfig, SnpmError};

use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

pub(super) fn read(path: &Path, config: &SnpmConfig) -> Result<Lockfile> {
    let data = fs::read_to_string(path).map_err(|source| SnpmError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;

    let probe: VersionProbe =
        serde_json::from_str(&data).map_err(|source| SnpmError::ParseJson {
            path: path.to_path_buf(),
            source,
        })?;

    let raw = match probe.lockfile_version {
        1 => parse_v1_as_v2(path, &data)?,
        2 | 3 => serde_json::from_str::<RawNpmLockfile>(&data).map_err(|source| {
            SnpmError::ParseJson {
                path: path.to_path_buf(),
                source,
            }
        })?,
        other => {
            return Err(SnpmError::Lockfile {
                path: path.to_path_buf(),
                reason: format!(
                    "{} lockfileVersion {other} is not supported; only npm lockfile v1/v2/v3 is currently supported",
                    lockfile_name(path)
                ),
            });
        }
    };

    let packages_by_path = normalize_packages(&raw.packages);
    let root_entry = packages_by_path
        .get("")
        .ok_or_else(|| SnpmError::Lockfile {
            path: path.to_path_buf(),
            reason: format!("{} is missing the root package entry", lockfile_name(path)),
        })?;
    let workspace_member_paths = collect_workspace_member_paths(root_entry, &packages_by_path);
    let link_targets_by_path = collect_link_targets(path, &packages_by_path)?;
    let link_names_by_target = collect_link_names_by_target(&link_targets_by_path);
    let package_entries = collect_package_entries(
        path,
        &packages_by_path,
        &workspace_member_paths,
        &link_names_by_target,
    )?;
    let install_path_to_package_key = build_install_path_map(
        path,
        &package_entries,
        &link_targets_by_path,
        &workspace_member_paths,
    )?;
    let packages = build_packages(
        path,
        config,
        &packages_by_path,
        &package_entries,
        &install_path_to_package_key,
        &link_targets_by_path,
        &workspace_member_paths,
    )?;
    let root = if root_entry.workspaces.is_empty() {
        build_root_from_entry(
            path,
            ".",
            "",
            root_entry,
            false,
            &install_path_to_package_key,
            &link_targets_by_path,
            &workspace_member_paths,
        )?
    } else {
        build_workspace_root(
            path,
            &packages_by_path,
            &workspace_member_paths,
            &install_path_to_package_key,
            &link_targets_by_path,
        )?
    };

    Ok(Lockfile {
        version: 1,
        root,
        packages,
    })
}

#[derive(Debug, Default, Deserialize)]
struct RawNpmLockfile {
    #[serde(default)]
    packages: BTreeMap<String, RawNpmPackage>,
}

#[derive(Debug, Deserialize)]
struct VersionProbe {
    #[serde(rename = "lockfileVersion")]
    lockfile_version: u32,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawNpmPackage {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    resolved: Option<String>,
    #[serde(default)]
    integrity: Option<String>,
    #[serde(default)]
    link: bool,
    #[serde(default)]
    dependencies: BTreeMap<String, String>,
    #[serde(default)]
    dev_dependencies: BTreeMap<String, String>,
    #[serde(default)]
    optional_dependencies: BTreeMap<String, String>,
    #[serde(default)]
    peer_dependencies: BTreeMap<String, String>,
    #[serde(default)]
    peer_dependencies_meta: BTreeMap<String, PeerDependencyMeta>,
    #[serde(default)]
    bundled_dependencies: Option<BundledDependencies>,
    #[serde(default)]
    bundle_dependencies: Option<BundledDependencies>,
    #[serde(default)]
    bin: Option<Value>,
    #[serde(default)]
    workspaces: Vec<String>,
}

#[derive(Debug)]
struct PackageEntry {
    install_path: String,
    name: String,
    version: String,
    lock_key: String,
}

#[derive(Debug)]
enum DependencyResolution {
    PackageKey(String),
    WorkspaceMember,
    Missing,
}

fn normalize_packages(raw: &BTreeMap<String, RawNpmPackage>) -> BTreeMap<String, RawNpmPackage> {
    raw.iter()
        .map(|(path, package)| (normalize_lockfile_path(path), package.clone()))
        .collect()
}

#[derive(Debug, Default, Deserialize)]
struct RawNpmV1Lockfile {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    dependencies: BTreeMap<String, RawNpmV1Dependency>,
}

#[derive(Debug, Default, Clone, Deserialize)]
struct RawNpmV1Dependency {
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    resolved: Option<String>,
    #[serde(default)]
    integrity: Option<String>,
    #[serde(default)]
    from: Option<String>,
    #[serde(default)]
    requires: BTreeMap<String, String>,
    #[serde(default)]
    dependencies: BTreeMap<String, RawNpmV1Dependency>,
    #[serde(default)]
    dev: bool,
    #[serde(default)]
    optional: bool,
    #[serde(default)]
    bundled: bool,
    #[serde(default)]
    bin: Option<Value>,
}

fn parse_v1_as_v2(path: &Path, data: &str) -> Result<RawNpmLockfile> {
    let v1: RawNpmV1Lockfile =
        serde_json::from_str(data).map_err(|source| SnpmError::ParseJson {
            path: path.to_path_buf(),
            source,
        })?;

    let mut packages = BTreeMap::new();
    flatten_v1_deps(&v1.dependencies, "", &mut packages);
    packages.insert(String::new(), build_v1_root_entry(path, &v1));

    Ok(RawNpmLockfile { packages })
}

fn flatten_v1_deps(
    deps: &BTreeMap<String, RawNpmV1Dependency>,
    parent_path: &str,
    out: &mut BTreeMap<String, RawNpmPackage>,
) {
    for (name, dep) in deps {
        let install_path = if parent_path.is_empty() {
            format!("node_modules/{name}")
        } else {
            format!("{parent_path}/node_modules/{name}")
        };

        let resolved = dep
            .resolved
            .clone()
            .or_else(|| dep.from.clone().filter(|value| value.contains("://")));

        let bundled_dependencies = collect_v1_bundled_children(&dep.dependencies);

        let entry = RawNpmPackage {
            name: Some(name.clone()),
            version: dep.version.clone(),
            resolved,
            integrity: dep.integrity.clone(),
            link: false,
            dependencies: dep.requires.clone(),
            dev_dependencies: BTreeMap::new(),
            optional_dependencies: BTreeMap::new(),
            // package-lock v1 entries carry no peerDependencies data.
            peer_dependencies: BTreeMap::new(),
            peer_dependencies_meta: BTreeMap::new(),
            bundled_dependencies,
            bundle_dependencies: None,
            bin: dep.bin.clone(),
            workspaces: Vec::new(),
        };

        out.insert(install_path.clone(), entry);
        flatten_v1_deps(&dep.dependencies, &install_path, out);
    }
}

fn collect_v1_bundled_children(
    children: &BTreeMap<String, RawNpmV1Dependency>,
) -> Option<BundledDependencies> {
    let names: Vec<String> = children
        .iter()
        .filter(|(_, child)| child.bundled)
        .map(|(name, _)| name.clone())
        .collect();
    if names.is_empty() {
        None
    } else {
        Some(BundledDependencies::List(names))
    }
}

fn build_v1_root_entry(path: &Path, v1: &RawNpmV1Lockfile) -> RawNpmPackage {
    let mut entry = RawNpmPackage {
        name: v1.name.clone(),
        version: v1.version.clone(),
        ..RawNpmPackage::default()
    };

    let pkg_json = path
        .parent()
        .map(|parent| parent.join("package.json"))
        .filter(|candidate| candidate.is_file());

    if let Some(pkg_json) = pkg_json
        && let Ok(content) = fs::read_to_string(&pkg_json)
        && let Ok(value) = serde_json::from_str::<Value>(&content)
    {
        populate_root_from_manifest(&mut entry, &value);
    }

    if entry.dependencies.is_empty()
        && entry.dev_dependencies.is_empty()
        && entry.optional_dependencies.is_empty()
    {
        for (name, dep) in &v1.dependencies {
            let spec = dep
                .from
                .clone()
                .or_else(|| dep.version.clone())
                .unwrap_or_else(|| "*".to_string());
            if dep.optional {
                entry.optional_dependencies.insert(name.clone(), spec);
            } else if dep.dev {
                entry.dev_dependencies.insert(name.clone(), spec);
            } else {
                entry.dependencies.insert(name.clone(), spec);
            }
        }
    }

    entry
}

fn populate_root_from_manifest(entry: &mut RawNpmPackage, manifest: &Value) {
    copy_dep_block(manifest, "dependencies", &mut entry.dependencies);
    copy_dep_block(manifest, "devDependencies", &mut entry.dev_dependencies);
    copy_dep_block(
        manifest,
        "optionalDependencies",
        &mut entry.optional_dependencies,
    );

    if let Some(workspaces) = manifest.get("workspaces") {
        entry.workspaces = workspaces_from_manifest(workspaces);
    }

    if entry.bin.is_none()
        && let Some(bin) = manifest.get("bin")
    {
        entry.bin = Some(bin.clone());
    }
}

fn copy_dep_block(manifest: &Value, key: &str, target: &mut BTreeMap<String, String>) {
    let Some(block) = manifest.get(key).and_then(Value::as_object) else {
        return;
    };
    for (name, spec) in block {
        if let Some(spec) = spec.as_str() {
            target.insert(name.clone(), spec.to_string());
        }
    }
}

fn workspaces_from_manifest(value: &Value) -> Vec<String> {
    if let Some(arr) = value.as_array() {
        return arr
            .iter()
            .filter_map(|item| item.as_str().map(str::to_string))
            .collect();
    }
    if let Some(obj) = value.as_object()
        && let Some(packages) = obj.get("packages").and_then(Value::as_array)
    {
        return packages
            .iter()
            .filter_map(|item| item.as_str().map(str::to_string))
            .collect();
    }
    Vec::new()
}

fn collect_workspace_member_paths(
    root_entry: &RawNpmPackage,
    packages_by_path: &BTreeMap<String, RawNpmPackage>,
) -> BTreeSet<String> {
    if root_entry.workspaces.is_empty() {
        return BTreeSet::new();
    }

    packages_by_path
        .keys()
        .filter(|path| !path.is_empty() && !contains_node_modules(path))
        .cloned()
        .collect()
}

fn collect_link_targets(
    path: &Path,
    packages_by_path: &BTreeMap<String, RawNpmPackage>,
) -> Result<BTreeMap<String, String>> {
    let mut targets = BTreeMap::new();

    for (install_path, package) in packages_by_path {
        if !package.link {
            continue;
        }

        let target = package
            .resolved
            .as_deref()
            .map(normalize_lockfile_path)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| SnpmError::Lockfile {
                path: path.to_path_buf(),
                reason: format!(
                    "{} link entry `{install_path}` is missing a resolved target",
                    lockfile_name(path)
                ),
            })?;

        targets.insert(install_path.clone(), target);
    }

    Ok(targets)
}

fn collect_link_names_by_target(
    link_targets_by_path: &BTreeMap<String, String>,
) -> BTreeMap<String, BTreeSet<String>> {
    let mut names_by_target = BTreeMap::new();

    for (install_path, target_path) in link_targets_by_path {
        let Some(name) = package_name_from_install_path(install_path) else {
            continue;
        };

        names_by_target
            .entry(target_path.clone())
            .or_insert_with(BTreeSet::new)
            .insert(name);
    }

    names_by_target
}

fn collect_package_entries(
    path: &Path,
    packages_by_path: &BTreeMap<String, RawNpmPackage>,
    workspace_member_paths: &BTreeSet<String>,
    link_names_by_target: &BTreeMap<String, BTreeSet<String>>,
) -> Result<Vec<PackageEntry>> {
    let mut entries = Vec::new();

    for (install_path, package) in packages_by_path {
        if install_path.is_empty() || package.link || workspace_member_paths.contains(install_path)
        {
            continue;
        }

        let name = resolve_package_name(
            path,
            install_path,
            package,
            link_names_by_target.get(install_path),
        )?;
        let version = package
            .version
            .clone()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| SnpmError::Lockfile {
                path: path.to_path_buf(),
                reason: format!(
                    "{} package entry `{install_path}` is missing a version",
                    lockfile_name(path)
                ),
            })?;

        entries.push(PackageEntry {
            lock_key: package_key(&name, &version),
            install_path: install_path.clone(),
            name,
            version,
        });
    }

    Ok(entries)
}

fn resolve_package_name(
    path: &Path,
    install_path: &str,
    package: &RawNpmPackage,
    link_names: Option<&BTreeSet<String>>,
) -> Result<String> {
    if let Some(name) = package.name.as_deref().filter(|value| !value.is_empty()) {
        return Ok(name.to_string());
    }

    if let Some(name) = package_name_from_install_path(install_path) {
        return Ok(name);
    }

    if let Some(names) = link_names {
        if names.len() == 1 {
            return Ok(names.iter().next().unwrap().clone());
        }

        if !names.is_empty() {
            return Err(SnpmError::Lockfile {
                path: path.to_path_buf(),
                reason: format!(
                    "{} local package entry `{install_path}` is linked from multiple names: {}",
                    lockfile_name(path),
                    names.iter().cloned().collect::<Vec<_>>().join(", ")
                ),
            });
        }
    }

    let fallback = install_path
        .rsplit('/')
        .next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| SnpmError::Lockfile {
            path: path.to_path_buf(),
            reason: format!(
                "{} package entry `{install_path}` is missing a package name",
                lockfile_name(path)
            ),
        })?;

    Ok(fallback.to_string())
}

fn build_install_path_map(
    path: &Path,
    package_entries: &[PackageEntry],
    link_targets_by_path: &BTreeMap<String, String>,
    workspace_member_paths: &BTreeSet<String>,
) -> Result<BTreeMap<String, String>> {
    let mut install_path_to_package_key = package_entries
        .iter()
        .map(|entry| (entry.install_path.clone(), entry.lock_key.clone()))
        .collect::<BTreeMap<_, _>>();

    for (install_path, target_path) in link_targets_by_path {
        if workspace_member_paths.contains(target_path) {
            continue;
        }

        let target_key = install_path_to_package_key.get(target_path).ok_or_else(|| {
            SnpmError::Lockfile {
                path: path.to_path_buf(),
                reason: format!(
                    "{} link entry `{install_path}` points to missing package entry `{target_path}`",
                    lockfile_name(path)
                ),
            }
        })?;

        install_path_to_package_key.insert(install_path.clone(), target_key.clone());
    }

    Ok(install_path_to_package_key)
}

fn build_packages(
    path: &Path,
    config: &SnpmConfig,
    packages_by_path: &BTreeMap<String, RawNpmPackage>,
    package_entries: &[PackageEntry],
    install_path_to_package_key: &BTreeMap<String, String>,
    link_targets_by_path: &BTreeMap<String, String>,
    workspace_member_paths: &BTreeSet<String>,
) -> Result<BTreeMap<String, LockPackage>> {
    let mut packages = BTreeMap::new();
    let project_root = path.parent().unwrap_or(Path::new("."));

    for entry in package_entries {
        let raw = packages_by_path
            .get(&entry.install_path)
            .ok_or_else(|| SnpmError::Lockfile {
                path: path.to_path_buf(),
                reason: format!(
                    "{} package entry `{}` disappeared during import",
                    lockfile_name(path),
                    entry.install_path
                ),
            })?;

        let mut dependencies = BTreeMap::new();
        for dep_name in raw.dependencies.keys() {
            let dep_key = match resolve_dependency(
                dep_name,
                &entry.install_path,
                install_path_to_package_key,
                link_targets_by_path,
                workspace_member_paths,
            ) {
                DependencyResolution::PackageKey(dep_key) => dep_key,
                DependencyResolution::WorkspaceMember | DependencyResolution::Missing => {
                    return Err(SnpmError::Lockfile {
                        path: path.to_path_buf(),
                        reason: format!(
                            "{} dependency `{dep_name}` from `{}` could not be resolved from the imported lockfile",
                            lockfile_name(path),
                            entry.install_path
                        ),
                    });
                }
            };

            dependencies.insert(dep_name.clone(), dep_key);
        }

        for dep_name in raw.optional_dependencies.keys() {
            if let DependencyResolution::PackageKey(dep_key) = resolve_dependency(
                dep_name,
                &entry.install_path,
                install_path_to_package_key,
                link_targets_by_path,
                workspace_member_paths,
            ) {
                dependencies.insert(dep_name.clone(), dep_key);
            }
        }

        let lock_package = LockPackage {
            name: entry.name.clone(),
            version: entry.version.clone(),
            tarball: build_package_tarball(project_root, config, entry, raw),
            integrity: raw.integrity.clone(),
            dependencies,
            peer_dependencies: super::required_peer_dependencies_with_meta(
                &raw.peer_dependencies,
                &raw.peer_dependencies_meta,
            ),
            bundled_dependencies: raw
                .bundled_dependencies
                .clone()
                .or_else(|| raw.bundle_dependencies.clone())
                .filter(|value| !value.is_empty()),
            has_bin: raw.bin.as_ref().is_some_and(has_bin),
            bin: None,
        };

        if let Some(existing) = packages.get(&entry.lock_key) {
            if existing != &lock_package {
                return Err(SnpmError::Lockfile {
                    path: path.to_path_buf(),
                    reason: format!(
                        "{} contains multiple package variants that collapse to the same snpm package key `{}`",
                        lockfile_name(path),
                        entry.lock_key
                    ),
                });
            }
            continue;
        }

        packages.insert(entry.lock_key.clone(), lock_package);
    }

    Ok(packages)
}

fn build_package_tarball(
    project_root: &Path,
    config: &SnpmConfig,
    entry: &PackageEntry,
    raw: &RawNpmPackage,
) -> String {
    if let Some(resolved) = raw.resolved.as_deref() {
        if resolved.starts_with("http://") || resolved.starts_with("https://") {
            return resolved.to_string();
        }

        if let Some(file_url) = local_file_url(project_root, resolved) {
            return file_url;
        }
    }

    if !contains_node_modules(&entry.install_path) {
        return format!(
            "file://{}",
            project_root.join(&entry.install_path).display()
        );
    }

    derive_registry_tarball(config, &entry.name, &entry.version).unwrap_or_default()
}

#[allow(clippy::too_many_arguments)]
fn build_root_from_entry(
    path: &Path,
    source_label: &str,
    source_install_path: &str,
    package: &RawNpmPackage,
    allow_workspace_members: bool,
    install_path_to_package_key: &BTreeMap<String, String>,
    link_targets_by_path: &BTreeMap<String, String>,
    workspace_member_paths: &BTreeSet<String>,
) -> Result<LockRoot> {
    let mut dependencies = BTreeMap::new();

    insert_root_block(
        path,
        source_label,
        source_install_path,
        &package.dependencies,
        false,
        allow_workspace_members,
        install_path_to_package_key,
        link_targets_by_path,
        workspace_member_paths,
        &mut dependencies,
    )?;
    insert_root_block(
        path,
        source_label,
        source_install_path,
        &package.dev_dependencies,
        false,
        allow_workspace_members,
        install_path_to_package_key,
        link_targets_by_path,
        workspace_member_paths,
        &mut dependencies,
    )?;
    insert_root_block(
        path,
        source_label,
        source_install_path,
        &package.optional_dependencies,
        true,
        allow_workspace_members,
        install_path_to_package_key,
        link_targets_by_path,
        workspace_member_paths,
        &mut dependencies,
    )?;

    Ok(LockRoot { dependencies })
}

fn build_workspace_root(
    path: &Path,
    packages_by_path: &BTreeMap<String, RawNpmPackage>,
    workspace_member_paths: &BTreeSet<String>,
    install_path_to_package_key: &BTreeMap<String, String>,
    link_targets_by_path: &BTreeMap<String, String>,
) -> Result<LockRoot> {
    let mut dependencies = BTreeMap::new();

    for member_path in workspace_member_paths {
        let member = packages_by_path
            .get(member_path)
            .ok_or_else(|| SnpmError::Lockfile {
                path: path.to_path_buf(),
                reason: format!(
                    "{} workspace member entry `{member_path}` disappeared during import",
                    lockfile_name(path)
                ),
            })?;

        insert_root_block(
            path,
            member_path,
            member_path,
            &member.dependencies,
            false,
            true,
            install_path_to_package_key,
            link_targets_by_path,
            workspace_member_paths,
            &mut dependencies,
        )?;
        insert_root_block(
            path,
            member_path,
            member_path,
            &member.dev_dependencies,
            false,
            true,
            install_path_to_package_key,
            link_targets_by_path,
            workspace_member_paths,
            &mut dependencies,
        )?;
        insert_root_block(
            path,
            member_path,
            member_path,
            &member.optional_dependencies,
            true,
            true,
            install_path_to_package_key,
            link_targets_by_path,
            workspace_member_paths,
            &mut dependencies,
        )?;
    }

    Ok(LockRoot { dependencies })
}

#[allow(clippy::too_many_arguments)]
fn insert_root_block(
    path: &Path,
    source_label: &str,
    source_install_path: &str,
    block: &BTreeMap<String, String>,
    optional: bool,
    allow_workspace_members: bool,
    install_path_to_package_key: &BTreeMap<String, String>,
    link_targets_by_path: &BTreeMap<String, String>,
    workspace_member_paths: &BTreeSet<String>,
    root: &mut BTreeMap<String, LockRootDependency>,
) -> Result<()> {
    for (dep_name, requested) in block {
        let incoming = match resolve_dependency(
            dep_name,
            source_install_path,
            install_path_to_package_key,
            link_targets_by_path,
            workspace_member_paths,
        ) {
            DependencyResolution::PackageKey(dep_key) => {
                if optional {
                    build_optional_root_dependency(dep_name, requested, Some(dep_key.as_str()))
                } else {
                    build_required_root_dependency(path, dep_name, requested, &dep_key)?
                }
            }
            DependencyResolution::WorkspaceMember if allow_workspace_members => continue,
            DependencyResolution::WorkspaceMember | DependencyResolution::Missing if optional => {
                build_optional_root_dependency(dep_name, requested, None)
            }
            DependencyResolution::Missing => {
                return Err(SnpmError::Lockfile {
                    path: path.to_path_buf(),
                    reason: format!(
                        "{} root dependency `{dep_name}` from `{source_label}` could not be resolved from the imported lockfile",
                        lockfile_name(path)
                    ),
                });
            }
            DependencyResolution::WorkspaceMember => {
                return Err(SnpmError::Lockfile {
                    path: path.to_path_buf(),
                    reason: format!(
                        "{} dependency `{dep_name}` from `{source_label}` resolves to an unsupported workspace member link",
                        lockfile_name(path)
                    ),
                });
            }
        };

        merge_root_dependency(path, source_label, dep_name, incoming, root)?;
    }

    Ok(())
}

fn build_optional_root_dependency(
    dep_name: &str,
    requested: &str,
    resolved: Option<&str>,
) -> LockRootDependency {
    if let Some(dep_key) = resolved
        && let Some((resolved_name, version)) = split_dep_key(dep_key)
    {
        return LockRootDependency {
            requested: requested.to_string(),
            package: (resolved_name != dep_name).then_some(resolved_name),
            version: Some(version),
            optional: true,
        };
    }

    LockRootDependency {
        requested: requested.to_string(),
        package: None,
        version: None,
        optional: true,
    }
}

fn build_required_root_dependency(
    path: &Path,
    dep_name: &str,
    requested: &str,
    dep_key: &str,
) -> Result<LockRootDependency> {
    let (resolved_name, version) = split_dep_key(dep_key).ok_or_else(|| SnpmError::Lockfile {
        path: path.to_path_buf(),
        reason: format!("unsupported resolved dependency key `{dep_key}`"),
    })?;

    Ok(LockRootDependency {
        requested: requested.to_string(),
        package: (resolved_name != dep_name).then_some(resolved_name),
        version: Some(version),
        optional: false,
    })
}

fn merge_root_dependency(
    path: &Path,
    source_label: &str,
    dep_name: &str,
    incoming: LockRootDependency,
    root: &mut BTreeMap<String, LockRootDependency>,
) -> Result<()> {
    let Some(existing) = root.get_mut(dep_name) else {
        root.insert(dep_name.to_string(), incoming);
        return Ok(());
    };

    if existing.requested != incoming.requested {
        return Err(root_conflict_error(
            path,
            source_label,
            dep_name,
            &existing.requested,
            &incoming.requested,
        ));
    }

    match (&existing.package, &incoming.package) {
        (Some(left), Some(right)) if left != right => {
            return Err(root_conflict_error(
                path,
                source_label,
                dep_name,
                left,
                right,
            ));
        }
        (None, Some(package)) => existing.package = Some(package.clone()),
        _ => {}
    }

    match (&existing.version, &incoming.version) {
        (Some(left), Some(right)) if left != right => {
            return Err(root_conflict_error(
                path,
                source_label,
                dep_name,
                left,
                right,
            ));
        }
        (None, Some(version)) => existing.version = Some(version.clone()),
        _ => {}
    }

    existing.optional &= incoming.optional;
    Ok(())
}

fn root_conflict_error(
    path: &Path,
    source_label: &str,
    dep_name: &str,
    left: &str,
    right: &str,
) -> SnpmError {
    SnpmError::Lockfile {
        path: path.to_path_buf(),
        reason: format!(
            "{} entry `{}` declares dependency `{dep_name}` with conflicting values `{left}` and `{right}`",
            lockfile_name(path),
            display_source_label(source_label)
        ),
    }
}

fn resolve_dependency(
    dep_name: &str,
    source_install_path: &str,
    install_path_to_package_key: &BTreeMap<String, String>,
    link_targets_by_path: &BTreeMap<String, String>,
    workspace_member_paths: &BTreeSet<String>,
) -> DependencyResolution {
    for candidate in dependency_candidate_paths(source_install_path, dep_name) {
        if let Some(dep_key) = install_path_to_package_key.get(&candidate) {
            return DependencyResolution::PackageKey(dep_key.clone());
        }

        if let Some(target_path) = link_targets_by_path.get(&candidate)
            && workspace_member_paths.contains(target_path)
        {
            return DependencyResolution::WorkspaceMember;
        }
    }

    DependencyResolution::Missing
}

fn dependency_candidate_paths(source_install_path: &str, dep_name: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    let mut seen = BTreeSet::new();
    let mut current = Some(source_install_path);

    while let Some(base) = current {
        let candidate = dependency_candidate(base, dep_name);
        if seen.insert(candidate.clone()) {
            candidates.push(candidate);
        }

        current = parent_path(base);
    }

    let root_candidate = format!("node_modules/{dep_name}");
    if seen.insert(root_candidate.clone()) {
        candidates.push(root_candidate);
    }

    candidates
}

fn dependency_candidate(base: &str, dep_name: &str) -> String {
    if base.is_empty() || base == "node_modules" {
        format!("node_modules/{dep_name}")
    } else if base.rsplit('/').next() == Some("node_modules") {
        format!("{base}/{dep_name}")
    } else {
        format!("{base}/node_modules/{dep_name}")
    }
}

fn parent_path(path: &str) -> Option<&str> {
    if path.is_empty() {
        return None;
    }

    match path.rsplit_once('/') {
        Some((parent, _)) => Some(parent),
        None => Some(""),
    }
}

fn package_name_from_install_path(install_path: &str) -> Option<String> {
    let install_path = normalize_lockfile_path(install_path);
    let idx = install_path.rfind("node_modules/")?;
    let tail = &install_path[idx + "node_modules/".len()..];

    if tail.is_empty() {
        return None;
    }

    if let Some(rest) = tail.strip_prefix('@') {
        let slash = rest.find('/')?;
        let scoped_end = slash + 1;
        let name_end = rest[scoped_end..]
            .find('/')
            .map(|value| scoped_end + value)
            .unwrap_or(rest.len());
        return Some(format!("@{}", &rest[..name_end]));
    }

    Some(tail.split('/').next().unwrap_or(tail).to_string())
}

fn build_local_file_url(project_root: &Path, relative_path: &str) -> String {
    format!("file://{}", project_root.join(relative_path).display())
}

fn local_file_url(project_root: &Path, resolved: &str) -> Option<String> {
    let resolved = resolved.trim();
    if resolved.is_empty() {
        return None;
    }

    if let Some(path) = resolved.strip_prefix("file://") {
        return Some(format!("file://{}", path));
    }

    if let Some(path) = resolved.strip_prefix("file:") {
        return Some(build_local_file_url(project_root, path));
    }

    if resolved.starts_with("http://")
        || resolved.starts_with("https://")
        || resolved.contains("://")
    {
        return None;
    }

    Some(build_local_file_url(project_root, resolved))
}

fn derive_registry_tarball(config: &SnpmConfig, name: &str, version: &str) -> Option<String> {
    if version.contains("://")
        || version.starts_with("file:")
        || version.starts_with("link:")
        || version.starts_with("workspace:")
        || version.contains('#')
    {
        return None;
    }

    let registry = scoped_registry_for_package(config, name);
    let encoded_name = encode_package_name(name);
    let tarball_name = name.rsplit('/').next().unwrap_or(name);

    Some(format!(
        "{}/{}/-/{}-{}.tgz",
        registry.trim_end_matches('/'),
        encoded_name,
        tarball_name,
        version
    ))
}

fn scoped_registry_for_package<'a>(config: &'a SnpmConfig, name: &str) -> &'a str {
    if let Some((scope, _)) = name.split_once('/')
        && scope.starts_with('@')
        && let Some(registry) = config.scoped_registries.get(scope)
    {
        return registry;
    }

    &config.default_registry
}

fn has_bin(value: &Value) -> bool {
    match value {
        Value::String(value) => !value.is_empty(),
        Value::Object(value) => !value.is_empty(),
        _ => false,
    }
}

fn contains_node_modules(path: &str) -> bool {
    path == "node_modules" || path.starts_with("node_modules/") || path.contains("/node_modules/")
}

fn normalize_lockfile_path(path: &str) -> String {
    let mut normalized = path.replace('\\', "/");
    while let Some(stripped) = normalized.strip_prefix("./") {
        normalized = stripped.to_string();
    }

    normalized.trim_end_matches('/').to_string()
}

fn display_source_label(label: &str) -> &str {
    if label.is_empty() { "." } else { label }
}

fn lockfile_name(path: &Path) -> String {
    path.file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("package-lock.json")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::read;
    use crate::config::SnpmConfig;

    use std::fs;

    fn test_config() -> SnpmConfig {
        SnpmConfig {
            registry_concurrency: 16,
            ..SnpmConfig::for_tests()
        }
    }

    #[test]
    fn imports_simple_package_lock_v3() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("package-lock.json");
        fs::write(
            &path,
            r#"{
  "name": "import-test",
  "version": "1.0.0",
  "lockfileVersion": 3,
  "packages": {
    "": {
      "name": "import-test",
      "version": "1.0.0",
      "dependencies": {
        "foo": "^1.0.0"
      },
      "devDependencies": {
        "bar": "^2.0.0"
      },
      "optionalDependencies": {
        "baz": "^3.0.0"
      }
    },
    "node_modules/foo": {
      "version": "1.2.0",
      "resolved": "https://registry.npmjs.org/foo/-/foo-1.2.0.tgz",
      "integrity": "sha512-foo",
      "dependencies": {
        "shared": "^4.0.0"
      }
    },
    "node_modules/bar": {
      "version": "2.1.0",
      "resolved": "https://registry.npmjs.org/bar/-/bar-2.1.0.tgz",
      "integrity": "sha512-bar",
      "bin": "bin.js"
    },
    "node_modules/baz": {
      "version": "3.1.0",
      "resolved": "https://registry.npmjs.org/baz/-/baz-3.1.0.tgz"
    },
    "node_modules/shared": {
      "version": "4.0.1",
      "resolved": "https://registry.npmjs.org/shared/-/shared-4.0.1.tgz",
      "bundledDependencies": [
        "nested"
      ]
    }
  }
}"#,
        )
        .unwrap();

        let lockfile = read(&path, &test_config()).unwrap();

        assert_eq!(lockfile.root.dependencies["foo"].requested, "^1.0.0");
        assert_eq!(
            lockfile.root.dependencies["foo"].version.as_deref(),
            Some("1.2.0")
        );
        assert_eq!(
            lockfile.root.dependencies["bar"].version.as_deref(),
            Some("2.1.0")
        );
        assert!(lockfile.root.dependencies["baz"].optional);
        assert_eq!(
            lockfile.packages["foo@1.2.0"].dependencies["shared"],
            "shared@4.0.1"
        );
        assert!(lockfile.packages["bar@2.1.0"].has_bin);
        assert!(
            lockfile.packages["shared@4.0.1"]
                .bundled_dependencies
                .is_some()
        );
    }

    #[test]
    fn imports_required_peer_dependencies() {
        // Peer data must survive the import: an empty peer map bypasses
        // peer-aware virtual-store placement, which shares the package
        // with no peer wired and breaks require() at runtime.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("package-lock.json");
        fs::write(
            &path,
            r#"{
  "name": "peer-test",
  "version": "1.0.0",
  "lockfileVersion": 3,
  "packages": {
    "": {
      "dependencies": {
        "react-dom": "^18.0.0",
        "react": "^18.0.0"
      }
    },
    "node_modules/react": {
      "version": "18.3.1",
      "resolved": "https://registry.npmjs.org/react/-/react-18.3.1.tgz"
    },
    "node_modules/react-dom": {
      "version": "18.3.1",
      "resolved": "https://registry.npmjs.org/react-dom/-/react-dom-18.3.1.tgz",
      "peerDependencies": {
        "react": "^18.3.1",
        "styling": ">=1"
      },
      "peerDependenciesMeta": {
        "styling": { "optional": true }
      }
    }
  }
}"#,
        )
        .unwrap();

        let lockfile = read(&path, &test_config()).unwrap();

        let react_dom = &lockfile.packages["react-dom@18.3.1"];
        assert_eq!(react_dom.peer_dependencies["react"], "^18.3.1");
        assert!(
            !react_dom.peer_dependencies.contains_key("styling"),
            "optional peers must be excluded, matching the resolver"
        );
        assert!(
            lockfile.packages["react@18.3.1"]
                .peer_dependencies
                .is_empty()
        );
    }

    #[test]
    fn imports_alias_root_dependency() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("package-lock.json");
        fs::write(
            &path,
            r#"{
  "name": "alias-test",
  "version": "1.0.0",
  "lockfileVersion": 3,
  "packages": {
    "": {
      "name": "alias-test",
      "version": "1.0.0",
      "dependencies": {
        "h3-v2": "npm:h3@2.0.1-rc.20"
      }
    },
    "node_modules/h3-v2": {
      "name": "h3",
      "version": "2.0.1-rc.20",
      "resolved": "https://registry.npmjs.org/h3/-/h3-2.0.1-rc.20.tgz",
      "integrity": "sha512-aliased"
    }
  }
}"#,
        )
        .unwrap();

        let lockfile = read(&path, &test_config()).unwrap();
        let dep = &lockfile.root.dependencies["h3-v2"];

        assert_eq!(dep.requested, "npm:h3@2.0.1-rc.20");
        assert_eq!(dep.package.as_deref(), Some("h3"));
        assert_eq!(dep.version.as_deref(), Some("2.0.1-rc.20"));
        assert_eq!(
            lockfile.packages["h3@2.0.1-rc.20"].tarball,
            "https://registry.npmjs.org/h3/-/h3-2.0.1-rc.20.tgz"
        );
    }

    #[test]
    fn resolves_nested_dependencies_using_npm_lookup_rules() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("package-lock.json");
        fs::write(
            &path,
            r#"{
  "name": "nested-test",
  "version": "1.0.0",
  "lockfileVersion": 3,
  "packages": {
    "": {
      "name": "nested-test",
      "version": "1.0.0",
      "dependencies": {
        "foo": "^1.0.0",
        "bar": "^2.0.0"
      }
    },
    "node_modules/foo": {
      "version": "1.0.0",
      "resolved": "https://registry.npmjs.org/foo/-/foo-1.0.0.tgz",
      "dependencies": {
        "bar": "^1.0.0"
      }
    },
    "node_modules/bar": {
      "version": "2.0.0",
      "resolved": "https://registry.npmjs.org/bar/-/bar-2.0.0.tgz"
    },
    "node_modules/foo/node_modules/bar": {
      "version": "1.0.0",
      "resolved": "https://registry.npmjs.org/bar/-/bar-1.0.0.tgz"
    }
  }
}"#,
        )
        .unwrap();

        let lockfile = read(&path, &test_config()).unwrap();

        assert_eq!(
            lockfile.packages["foo@1.0.0"].dependencies["bar"],
            "bar@1.0.0"
        );
        assert_eq!(
            lockfile.root.dependencies["bar"].version.as_deref(),
            Some("2.0.0")
        );
    }

    #[test]
    fn imports_local_file_dependencies() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("package-lock.json");
        fs::write(
            &path,
            r#"{
  "name": "local-test",
  "version": "1.0.0",
  "lockfileVersion": 3,
  "packages": {
    "": {
      "name": "local-test",
      "version": "1.0.0",
      "dependencies": {
        "local-pkg": "file:packages/local-pkg"
      }
    },
    "node_modules/local-pkg": {
      "resolved": "packages/local-pkg",
      "link": true
    },
    "packages/local-pkg": {
      "version": "1.0.0",
      "dependencies": {
        "left-pad": "^1.3.0"
      }
    },
    "node_modules/left-pad": {
      "version": "1.3.0",
      "resolved": "https://registry.npmjs.org/left-pad/-/left-pad-1.3.0.tgz"
    }
  }
}"#,
        )
        .unwrap();

        let lockfile = read(&path, &test_config()).unwrap();

        assert_eq!(
            lockfile.root.dependencies["local-pkg"].version.as_deref(),
            Some("1.0.0")
        );
        assert_eq!(
            lockfile.packages["local-pkg@1.0.0"].tarball,
            format!("file://{}", dir.path().join("packages/local-pkg").display())
        );
        assert_eq!(
            lockfile.packages["local-pkg@1.0.0"].dependencies["left-pad"],
            "left-pad@1.3.0"
        );
    }

    #[test]
    fn imports_shared_workspace_package_lock_external_dependencies() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("package-lock.json");
        fs::write(
            &path,
            r#"{
  "name": "workspace-import",
  "version": "1.0.0",
  "lockfileVersion": 2,
  "packages": {
    "": {
      "name": "workspace-import",
      "version": "1.0.0",
      "workspaces": [
        "packages/**"
      ]
    },
    "node_modules/bar": {
      "resolved": "packages/bar",
      "link": true
    },
    "node_modules/foo": {
      "resolved": "packages/foo",
      "link": true
    },
    "node_modules/is-negative": {
      "version": "1.0.1",
      "resolved": "https://registry.npmjs.org/is-negative/-/is-negative-1.0.1.tgz"
    },
    "node_modules/is-positive": {
      "version": "1.0.0",
      "resolved": "https://registry.npmjs.org/is-positive/-/is-positive-1.0.0.tgz"
    },
    "packages/bar": {
      "version": "0.0.0",
      "dependencies": {
        "is-negative": "^1.0.0"
      }
    },
    "packages/foo": {
      "version": "0.0.0",
      "dependencies": {
        "is-positive": "^1.0.0"
      }
    }
  }
}"#,
        )
        .unwrap();

        let lockfile = read(&path, &test_config()).unwrap();

        assert_eq!(lockfile.root.dependencies.len(), 2);
        assert!(lockfile.root.dependencies.contains_key("is-negative"));
        assert!(lockfile.root.dependencies.contains_key("is-positive"));
        assert_eq!(lockfile.packages.len(), 2);
        assert!(!lockfile.packages.contains_key("bar@0.0.0"));
        assert!(!lockfile.packages.contains_key("foo@0.0.0"));
    }

    #[test]
    fn skips_workspace_member_links_inside_shared_package_lock() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("package-lock.json");
        fs::write(
            &path,
            r#"{
  "name": "workspace-local-links",
  "version": "1.0.0",
  "lockfileVersion": 2,
  "packages": {
    "": {
      "name": "workspace-local-links",
      "version": "1.0.0",
      "workspaces": [
        "packages/**"
      ]
    },
    "node_modules/bar": {
      "resolved": "packages/bar",
      "link": true
    },
    "node_modules/foo": {
      "resolved": "packages/foo",
      "link": true
    },
    "node_modules/is-negative": {
      "version": "1.0.1",
      "resolved": "https://registry.npmjs.org/is-negative/-/is-negative-1.0.1.tgz"
    },
    "packages/bar": {
      "version": "1.0.0",
      "dependencies": {
        "is-negative": "^1.0.0"
      }
    },
    "packages/foo": {
      "version": "1.0.0",
      "dependencies": {
        "bar": "^1.0.0"
      }
    }
  }
}"#,
        )
        .unwrap();

        let lockfile = read(&path, &test_config()).unwrap();

        assert_eq!(lockfile.root.dependencies.len(), 1);
        assert!(lockfile.root.dependencies.contains_key("is-negative"));
        assert!(!lockfile.root.dependencies.contains_key("bar"));
        assert!(!lockfile.packages.contains_key("foo@1.0.0"));
        assert!(!lockfile.packages.contains_key("bar@1.0.0"));
    }

    #[test]
    fn imports_empty_npm_lockfile_v1() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("package-lock.json");
        fs::write(
            &path,
            r#"{
  "name": "legacy-test",
  "lockfileVersion": 1,
  "dependencies": {}
}"#,
        )
        .unwrap();

        let lockfile = read(&path, &test_config()).unwrap();
        assert_eq!(lockfile.version, 1);
        assert!(lockfile.root.dependencies.is_empty());
        assert!(lockfile.packages.is_empty());
    }

    #[test]
    fn imports_npm_lockfile_v1_flat() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("package-lock.json");
        fs::write(
            dir.path().join("package.json"),
            r#"{
  "name": "v1-flat",
  "version": "1.0.0",
  "dependencies": { "foo": "^1.0.0" },
  "devDependencies": { "bar": "^2.0.0" }
}"#,
        )
        .unwrap();
        fs::write(
            &path,
            r#"{
  "name": "v1-flat",
  "version": "1.0.0",
  "lockfileVersion": 1,
  "requires": true,
  "dependencies": {
    "foo": {
      "version": "1.0.0",
      "resolved": "https://registry.npmjs.org/foo/-/foo-1.0.0.tgz",
      "integrity": "sha512-foo"
    },
    "bar": {
      "version": "2.0.0",
      "resolved": "https://registry.npmjs.org/bar/-/bar-2.0.0.tgz",
      "integrity": "sha512-bar",
      "dev": true,
      "requires": {
        "foo": "^1.0.0"
      }
    }
  }
}"#,
        )
        .unwrap();

        let lockfile = read(&path, &test_config()).unwrap();

        assert_eq!(lockfile.version, 1);
        assert_eq!(lockfile.root.dependencies.len(), 2);
        assert_eq!(
            lockfile
                .root
                .dependencies
                .get("foo")
                .unwrap()
                .version
                .as_deref(),
            Some("1.0.0"),
        );
        assert_eq!(
            lockfile
                .root
                .dependencies
                .get("bar")
                .unwrap()
                .version
                .as_deref(),
            Some("2.0.0"),
        );
        assert!(lockfile.packages.contains_key("foo@1.0.0"));
        assert!(lockfile.packages.contains_key("bar@2.0.0"));

        let bar = lockfile.packages.get("bar@2.0.0").unwrap();
        assert_eq!(bar.dependencies.get("foo").unwrap(), "foo@1.0.0");
    }

    #[test]
    fn imports_npm_lockfile_v1_nested_shadow() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("package-lock.json");
        fs::write(
            dir.path().join("package.json"),
            r#"{
  "name": "v1-nested",
  "version": "1.0.0",
  "dependencies": { "alpha": "^1", "beta": "^1" }
}"#,
        )
        .unwrap();
        fs::write(
            &path,
            r#"{
  "name": "v1-nested",
  "version": "1.0.0",
  "lockfileVersion": 1,
  "requires": true,
  "dependencies": {
    "alpha": {
      "version": "1.0.0",
      "resolved": "https://registry.npmjs.org/alpha/-/alpha-1.0.0.tgz",
      "integrity": "sha512-alpha",
      "requires": { "shared": "^1.0.0" },
      "dependencies": {
        "shared": {
          "version": "1.0.0",
          "resolved": "https://registry.npmjs.org/shared/-/shared-1.0.0.tgz",
          "integrity": "sha512-sharedv1"
        }
      }
    },
    "beta": {
      "version": "1.0.0",
      "resolved": "https://registry.npmjs.org/beta/-/beta-1.0.0.tgz",
      "integrity": "sha512-beta",
      "requires": { "shared": "^2.0.0" },
      "dependencies": {
        "shared": {
          "version": "2.0.0",
          "resolved": "https://registry.npmjs.org/shared/-/shared-2.0.0.tgz",
          "integrity": "sha512-sharedv2"
        }
      }
    }
  }
}"#,
        )
        .unwrap();

        let lockfile = read(&path, &test_config()).unwrap();
        assert_eq!(lockfile.root.dependencies.len(), 2);
        let alpha = lockfile.packages.get("alpha@1.0.0").unwrap();
        let beta = lockfile.packages.get("beta@1.0.0").unwrap();
        assert_eq!(alpha.dependencies.get("shared").unwrap(), "shared@1.0.0");
        assert_eq!(beta.dependencies.get("shared").unwrap(), "shared@2.0.0");
        assert!(lockfile.packages.contains_key("shared@1.0.0"));
        assert!(lockfile.packages.contains_key("shared@2.0.0"));
    }

    #[test]
    fn imports_npm_lockfile_v1_without_package_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("package-lock.json");
        fs::write(
            &path,
            r#"{
  "name": "no-manifest",
  "version": "0.1.0",
  "lockfileVersion": 1,
  "requires": true,
  "dependencies": {
    "foo": {
      "version": "1.0.0",
      "resolved": "https://registry.npmjs.org/foo/-/foo-1.0.0.tgz",
      "integrity": "sha512-foo"
    },
    "tool": {
      "version": "0.1.0",
      "resolved": "https://registry.npmjs.org/tool/-/tool-0.1.0.tgz",
      "integrity": "sha512-tool",
      "dev": true
    }
  }
}"#,
        )
        .unwrap();

        let lockfile = read(&path, &test_config()).unwrap();
        assert_eq!(lockfile.root.dependencies.len(), 2);
        assert!(lockfile.root.dependencies.contains_key("foo"));
        assert!(lockfile.root.dependencies.contains_key("tool"));
    }

    #[test]
    fn rejects_unknown_npm_lockfile_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("package-lock.json");
        fs::write(
            &path,
            r#"{
  "name": "future-test",
  "lockfileVersion": 99,
  "packages": {}
}"#,
        )
        .unwrap();

        let error = read(&path, &test_config()).unwrap_err();
        assert!(matches!(
            error,
            crate::SnpmError::Lockfile { reason, .. } if reason.contains("lockfileVersion 99")
        ));
    }
}
