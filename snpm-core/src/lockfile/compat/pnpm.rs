use super::super::keys::{package_key, split_dep_key};
use super::super::types::{LockPackage, LockRoot, LockRootDependency, Lockfile};
use crate::protocols::encode_package_name;
use crate::registry::BundledDependencies;
use crate::{Result, SnpmConfig, SnpmError};

use serde::Deserialize;
use serde_yaml::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

pub(super) fn read(path: &Path, config: &SnpmConfig) -> Result<Lockfile> {
    let data = fs::read_to_string(path).map_err(|source| SnpmError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;
    let content = extract_main_document(data.trim_start_matches('\u{feff}'));
    let raw: RawPnpmLockfile =
        serde_yaml::from_str(content).map_err(|error| SnpmError::Lockfile {
            path: path.to_path_buf(),
            reason: format!("failed to parse pnpm-lock.yaml: {error}"),
        })?;

    let version =
        stringify_lockfile_version(&raw.lockfile_version).ok_or_else(|| SnpmError::Lockfile {
            path: path.to_path_buf(),
            reason: "pnpm-lock.yaml is missing a supported lockfileVersion".into(),
        })?;
    if !version.starts_with('9') {
        return Err(SnpmError::Lockfile {
            path: path.to_path_buf(),
            reason: format!(
                "pnpm lockfile version {version} is not supported yet; only pnpm v9 lockfiles are currently supported"
            ),
        });
    }

    validate_importers(path, &raw.importers)?;

    let package_entries = collect_package_entries(path, &raw)?;
    let dep_path_to_package_key = build_dep_path_map(&package_entries);
    let packages = build_packages(
        path,
        config,
        &raw,
        &package_entries,
        &dep_path_to_package_key,
    )?;
    let root = build_root(path, &raw, &dep_path_to_package_key)?;

    Ok(Lockfile {
        version: 1,
        root,
        packages,
    })
}

#[derive(Debug, Deserialize)]
struct RawPnpmLockfile {
    #[serde(rename = "lockfileVersion")]
    lockfile_version: Value,
    #[serde(default)]
    importers: BTreeMap<String, RawImporter>,
    #[serde(default)]
    packages: BTreeMap<String, RawPackageInfo>,
    #[serde(default)]
    snapshots: BTreeMap<String, RawSnapshot>,
}

#[derive(Debug, Default, Deserialize)]
struct RawImporter {
    #[serde(default)]
    specifiers: BTreeMap<String, String>,
    #[serde(default)]
    dependencies: BTreeMap<String, RawImporterDependency>,
    #[serde(default, rename = "devDependencies")]
    dev_dependencies: BTreeMap<String, RawImporterDependency>,
    #[serde(default, rename = "optionalDependencies")]
    optional_dependencies: BTreeMap<String, RawImporterDependency>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum RawImporterDependency {
    Inline { specifier: String, version: String },
    Version(String),
}

impl RawImporterDependency {
    fn version(&self) -> &str {
        match self {
            RawImporterDependency::Inline { version, .. } => version,
            RawImporterDependency::Version(version) => version,
        }
    }

    fn specifier<'a>(
        &'a self,
        name: &str,
        specifiers: &'a BTreeMap<String, String>,
    ) -> Option<&'a str> {
        match self {
            RawImporterDependency::Inline { specifier, .. } => Some(specifier.as_str()),
            RawImporterDependency::Version(_) => specifiers.get(name).map(String::as_str),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
struct RawPackageInfo {
    #[serde(default)]
    resolution: RawResolution,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default, rename = "bundledDependencies")]
    bundled_dependencies: Option<BundledDependencies>,
    #[serde(default, rename = "hasBin")]
    has_bin: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct RawResolution {
    #[serde(default)]
    tarball: Option<String>,
    #[serde(default)]
    integrity: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct RawSnapshot {
    #[serde(default)]
    dependencies: BTreeMap<String, String>,
    #[serde(default, rename = "optionalDependencies")]
    optional_dependencies: BTreeMap<String, String>,
}

#[derive(Debug)]
struct PackageEntry {
    source_key: String,
    lookup_key: String,
    name: String,
    version: String,
    lock_key: String,
}

fn extract_main_document(content: &str) -> &str {
    const START: &str = "---\n";
    const SEP: &str = "\n---\n";

    if !content.starts_with(START) {
        return content;
    }

    let Some(idx) = content[START.len()..].find(SEP) else {
        return "";
    };

    &content[idx + START.len() + SEP.len()..]
}

fn stringify_lockfile_version(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn validate_importers(path: &Path, importers: &BTreeMap<String, RawImporter>) -> Result<()> {
    let importer_keys: BTreeSet<_> = importers.keys().map(String::as_str).collect();
    if importer_keys.is_empty() {
        return Err(SnpmError::Lockfile {
            path: path.to_path_buf(),
            reason: "pnpm-lock.yaml is missing the root importer".into(),
        });
    }

    if importer_keys.len() > 1
        || !importer_keys
            .iter()
            .all(|key| *key == "." || key.is_empty())
    {
        return Err(SnpmError::Lockfile {
            path: path.to_path_buf(),
            reason: "workspace pnpm lockfile import is not supported yet; this first pass only supports single-package projects".into(),
        });
    }

    Ok(())
}

fn collect_package_entries(path: &Path, raw: &RawPnpmLockfile) -> Result<Vec<PackageEntry>> {
    let source_keys: Vec<String> = if raw.snapshots.is_empty() {
        raw.packages.keys().cloned().collect()
    } else {
        raw.snapshots.keys().cloned().collect()
    };

    let mut entries = Vec::new();
    for source_key in source_keys {
        let lookup_key = canonical_lookup_key(&source_key, &raw.packages);
        let package_info = raw.packages.get(&lookup_key);
        let (name, version) = parse_package_identity(path, &lookup_key, package_info)?;
        let lock_key = package_key(&name, &version);

        entries.push(PackageEntry {
            source_key,
            lookup_key,
            name,
            version,
            lock_key,
        });
    }

    Ok(entries)
}

fn canonical_lookup_key(source_key: &str, packages: &BTreeMap<String, RawPackageInfo>) -> String {
    let stripped = strip_peer_suffix(source_key);
    if packages.contains_key(stripped) {
        stripped.to_string()
    } else {
        source_key.to_string()
    }
}

fn parse_package_identity(
    path: &Path,
    lookup_key: &str,
    package_info: Option<&RawPackageInfo>,
) -> Result<(String, String)> {
    let (parsed_name, parsed_version) =
        split_dep_key(lookup_key).ok_or_else(|| SnpmError::Lockfile {
            path: path.to_path_buf(),
            reason: format!("unsupported pnpm package key `{lookup_key}`"),
        })?;

    let name = package_info
        .and_then(|info| info.name.clone())
        .unwrap_or(parsed_name);
    let version = package_info
        .and_then(|info| info.version.clone())
        .filter(|value| !value.is_empty())
        .unwrap_or(parsed_version);

    Ok((name, version))
}

fn build_dep_path_map(entries: &[PackageEntry]) -> BTreeMap<String, String> {
    let mut dep_paths = BTreeMap::new();

    for entry in entries {
        dep_paths.insert(entry.source_key.clone(), entry.lock_key.clone());
        dep_paths
            .entry(entry.lookup_key.clone())
            .or_insert_with(|| entry.lock_key.clone());
    }

    dep_paths
}

fn build_packages(
    path: &Path,
    config: &SnpmConfig,
    raw: &RawPnpmLockfile,
    entries: &[PackageEntry],
    dep_path_to_package_key: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, LockPackage>> {
    let mut packages = BTreeMap::new();

    for entry in entries {
        let snapshot = raw
            .snapshots
            .get(&entry.source_key)
            .or_else(|| raw.snapshots.get(&entry.lookup_key));
        let package_info = raw.packages.get(&entry.lookup_key);

        let mut dependencies = BTreeMap::new();
        if let Some(snapshot) = snapshot {
            for (dep_name, dep_ref) in &snapshot.dependencies {
                let dep_key = resolve_dependency_key(dep_name, dep_ref, dep_path_to_package_key)
                    .ok_or_else(|| SnpmError::Lockfile {
                        path: path.to_path_buf(),
                        reason: format!(
                            "pnpm dependency `{dep_name}` -> `{dep_ref}` could not be resolved from the imported lockfile"
                        ),
                    })?;
                dependencies.insert(dep_name.clone(), dep_key);
            }

            for (dep_name, dep_ref) in &snapshot.optional_dependencies {
                if let Some(dep_key) =
                    resolve_dependency_key(dep_name, dep_ref, dep_path_to_package_key)
                {
                    dependencies.insert(dep_name.clone(), dep_key);
                }
            }
        }

        let lock_package = LockPackage {
            name: entry.name.clone(),
            version: entry.version.clone(),
            tarball: package_info
                .and_then(|info| info.resolution.tarball.clone())
                .or_else(|| {
                    derive_registry_tarball(config, &entry.name, &entry.version, &entry.lookup_key)
                })
                .unwrap_or_default(),
            integrity: package_info.and_then(|info| info.resolution.integrity.clone()),
            dependencies,
            bundled_dependencies: package_info.and_then(|info| info.bundled_dependencies.clone()),
            has_bin: package_info.map(|info| info.has_bin).unwrap_or(false),
        };

        if let Some(existing) = packages.get(&entry.lock_key) {
            if existing != &lock_package {
                return Err(SnpmError::Lockfile {
                    path: path.to_path_buf(),
                    reason: format!(
                        "pnpm lockfile contains multiple peer-context variants for `{}` that collapse to the same snpm package key `{}`",
                        entry.lookup_key, entry.lock_key
                    ),
                });
            }
            continue;
        }

        packages.insert(entry.lock_key.clone(), lock_package);
    }

    Ok(packages)
}

fn build_root(
    path: &Path,
    raw: &RawPnpmLockfile,
    dep_path_to_package_key: &BTreeMap<String, String>,
) -> Result<LockRoot> {
    let importer = raw
        .importers
        .get(".")
        .or_else(|| raw.importers.get(""))
        .ok_or_else(|| SnpmError::Lockfile {
            path: path.to_path_buf(),
            reason: "pnpm-lock.yaml is missing the root importer".into(),
        })?;

    let mut dependencies = BTreeMap::new();

    insert_root_block(
        path,
        importer,
        &importer.dependencies,
        false,
        dep_path_to_package_key,
        &mut dependencies,
    )?;
    insert_root_block(
        path,
        importer,
        &importer.dev_dependencies,
        false,
        dep_path_to_package_key,
        &mut dependencies,
    )?;
    insert_root_block(
        path,
        importer,
        &importer.optional_dependencies,
        true,
        dep_path_to_package_key,
        &mut dependencies,
    )?;

    Ok(LockRoot { dependencies })
}

fn insert_root_block(
    path: &Path,
    importer: &RawImporter,
    block: &BTreeMap<String, RawImporterDependency>,
    optional: bool,
    dep_path_to_package_key: &BTreeMap<String, String>,
    root: &mut BTreeMap<String, LockRootDependency>,
) -> Result<()> {
    for (dep_name, dep) in block {
        let requested = dep
            .specifier(dep_name, &importer.specifiers)
            .unwrap_or(dep.version())
            .to_string();
        let resolved = resolve_dependency_key(dep_name, dep.version(), dep_path_to_package_key);

        if optional {
            root.entry(dep_name.clone()).or_insert_with(|| {
                if let Some(dep_key) = resolved.as_ref() {
                    if let Some((resolved_name, version)) = split_dep_key(dep_key) {
                        return LockRootDependency {
                            requested: requested.clone(),
                            package: (resolved_name != *dep_name).then_some(resolved_name),
                            version: Some(version),
                            optional: true,
                        };
                    }
                }

                LockRootDependency {
                    requested: requested.clone(),
                    package: None,
                    version: None,
                    optional: true,
                }
            });
            continue;
        }

        let dep_key = resolved.ok_or_else(|| SnpmError::Lockfile {
            path: path.to_path_buf(),
            reason: format!(
                "pnpm root dependency `{dep_name}` -> `{}` could not be resolved from the imported lockfile",
                dep.version()
            ),
        })?;
        let (resolved_name, version) =
            split_dep_key(&dep_key).ok_or_else(|| SnpmError::Lockfile {
                path: path.to_path_buf(),
                reason: format!("unsupported resolved dependency key `{dep_key}`"),
            })?;

        root.insert(
            dep_name.clone(),
            LockRootDependency {
                requested,
                package: (resolved_name != *dep_name).then_some(resolved_name),
                version: Some(version),
                optional: false,
            },
        );
    }

    Ok(())
}

fn resolve_dependency_key(
    dep_name: &str,
    dep_ref: &str,
    dep_path_to_package_key: &BTreeMap<String, String>,
) -> Option<String> {
    let dep_ref = strip_peer_suffix(dep_ref);
    let candidate = if dep_path_to_package_key.contains_key(dep_ref) {
        dep_ref.to_string()
    } else {
        format!("{dep_name}@{dep_ref}")
    };

    dep_path_to_package_key.get(&candidate).cloned()
}

fn strip_peer_suffix(dep_ref: &str) -> &str {
    dep_ref
        .split_once('(')
        .map(|(head, _)| head)
        .unwrap_or(dep_ref)
}

fn derive_registry_tarball(
    config: &SnpmConfig,
    name: &str,
    version: &str,
    lookup_key: &str,
) -> Option<String> {
    if !looks_like_registry_package(lookup_key) {
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

fn looks_like_registry_package(lookup_key: &str) -> bool {
    let Some((_, version)) = split_dep_key(lookup_key) else {
        return false;
    };

    !version.contains("://")
        && !version.starts_with("file:")
        && !version.starts_with("link:")
        && !version.starts_with("workspace:")
        && !version.contains('#')
}

fn scoped_registry_for_package<'a>(config: &'a SnpmConfig, name: &str) -> &'a str {
    if let Some((scope, _)) = name.split_once('/') {
        if scope.starts_with('@') {
            if let Some(registry) = config.scoped_registries.get(scope) {
                return registry;
            }
        }
    }

    &config.default_registry
}

#[cfg(test)]
mod tests {
    use super::read;
    use crate::config::{AuthScheme, HoistingMode, LinkBackend, SnpmConfig};

    use std::collections::{BTreeMap, BTreeSet};

    fn test_config() -> SnpmConfig {
        SnpmConfig {
            cache_dir: "/tmp/cache".into(),
            data_dir: "/tmp/data".into(),
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
            registry_concurrency: 16,
            verbose: false,
            log_file: None,
        }
    }

    #[test]
    fn imports_simple_pnpm_v9_lockfile() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pnpm-lock.yaml");
        std::fs::write(
            &path,
            r#"lockfileVersion: '9.0'
importers:
  .:
    dependencies:
      is-positive:
        specifier: ^1.0.0
        version: 1.0.0
packages:
  is-positive@1.0.0:
    resolution:
      integrity: sha512-abc
snapshots:
  is-positive@1.0.0: {}
"#,
        )
        .unwrap();

        let lockfile = read(&path, &test_config()).unwrap();
        let root_dep = &lockfile.root.dependencies["is-positive"];
        let pkg = &lockfile.packages["is-positive@1.0.0"];

        assert_eq!(root_dep.requested, "^1.0.0");
        assert_eq!(root_dep.version.as_deref(), Some("1.0.0"));
        assert_eq!(
            pkg.tarball,
            "https://registry.npmjs.org/is-positive/-/is-positive-1.0.0.tgz"
        );
        assert_eq!(pkg.integrity.as_deref(), Some("sha512-abc"));
    }

    #[test]
    fn imports_alias_root_dependency() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pnpm-lock.yaml");
        std::fs::write(
            &path,
            r#"lockfileVersion: '9.0'
importers:
  .:
    dependencies:
      alias:
        specifier: npm:@scope/real@^1.2.3
        version: '@scope/real@1.2.3'
packages:
  '@scope/real@1.2.3':
    resolution:
      integrity: sha512-abc
snapshots:
  '@scope/real@1.2.3': {}
"#,
        )
        .unwrap();

        let lockfile = read(&path, &test_config()).unwrap();
        let root_dep = &lockfile.root.dependencies["alias"];

        assert_eq!(root_dep.package.as_deref(), Some("@scope/real"));
        assert_eq!(root_dep.version.as_deref(), Some("1.2.3"));
    }

    #[test]
    fn rejects_workspace_lockfiles_for_now() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pnpm-lock.yaml");
        std::fs::write(
            &path,
            r#"lockfileVersion: '9.0'
importers:
  project-a: {}
  project-b: {}
"#,
        )
        .unwrap();

        let err = read(&path, &test_config()).unwrap_err();
        assert!(err.to_string().contains("workspace pnpm lockfile import"));
    }

    #[test]
    fn rejects_duplicate_peer_variants_that_collapse() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pnpm-lock.yaml");
        std::fs::write(
            &path,
            r#"lockfileVersion: '9.0'
importers:
  .:
    dependencies:
      peer-demo:
        specifier: ^1.0.0
        version: 1.0.0(peer-a@1.0.0)
packages:
  peer-demo@1.0.0:
    resolution:
      integrity: sha512-abc
  peer-a@1.0.0:
    resolution:
      integrity: sha512-peer-a
  peer-b@1.0.0:
    resolution:
      integrity: sha512-peer-b
snapshots:
  peer-demo@1.0.0(peer-a@1.0.0):
    dependencies:
      peer-a: 1.0.0
  peer-demo@1.0.0(peer-b@1.0.0):
    dependencies:
      peer-b: 1.0.0
  peer-a@1.0.0: {}
  peer-b@1.0.0: {}
"#,
        )
        .unwrap();

        let err = read(&path, &test_config()).unwrap_err();
        assert!(err.to_string().contains("multiple peer-context variants"));
    }

    #[test]
    fn reads_main_document_from_combined_env_lockfile() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pnpm-lock.yaml");
        std::fs::write(
            &path,
            r#"---
lockfileVersion: '1.0'
importers:
  .: {}
packages: {}
---
lockfileVersion: '9.0'
importers:
  .:
    dependencies:
      is-positive:
        specifier: ^1.0.0
        version: 1.0.0
packages:
  is-positive@1.0.0:
    resolution:
      integrity: sha512-abc
snapshots:
  is-positive@1.0.0: {}
"#,
        )
        .unwrap();

        let lockfile = read(&path, &test_config()).unwrap();
        assert!(lockfile.root.dependencies.contains_key("is-positive"));
    }
}
