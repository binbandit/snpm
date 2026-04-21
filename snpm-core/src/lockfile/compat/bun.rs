use super::super::keys::{package_key, split_dep_key};
use super::super::types::{LockPackage, LockRoot, LockRootDependency, Lockfile};
use crate::project::BinField;
use crate::protocols::encode_package_name;
use crate::{Result, SnpmConfig, SnpmError};

use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

pub(super) fn read(path: &Path, config: &SnpmConfig) -> Result<Lockfile> {
    let raw_content = fs::read_to_string(path).map_err(|source| SnpmError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;
    let cleaned = strip_jsonc(&raw_content);
    let raw: RawBunLockfile =
        serde_json::from_str(&cleaned).map_err(|source| SnpmError::ParseJson {
            path: path.to_path_buf(),
            source,
        })?;

    if raw.lockfile_version != 1 {
        return Err(SnpmError::Lockfile {
            path: path.to_path_buf(),
            reason: format!(
                "bun.lock lockfileVersion {} is not supported yet; only bun text lockfile v1 is currently supported",
                raw.lockfile_version
            ),
        });
    }

    let entries = decode_entries(path, &raw.packages)?;
    let key_info = build_key_info(path, &entries)?;
    let workspace_names = collect_workspace_names(&raw.workspaces);
    let packages = build_packages(path, config, &entries, &key_info)?;
    let root = build_root(path, &raw.workspaces, &key_info, &workspace_names)?;

    Ok(Lockfile {
        version: 1,
        root,
        packages,
    })
}

#[derive(Debug, Deserialize)]
struct RawBunLockfile {
    #[serde(rename = "lockfileVersion")]
    lockfile_version: u32,
    #[serde(default)]
    workspaces: BTreeMap<String, RawBunWorkspace>,
    #[serde(default)]
    packages: BTreeMap<String, Vec<Value>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawBunWorkspace {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    dependencies: BTreeMap<String, String>,
    #[serde(default)]
    dev_dependencies: BTreeMap<String, String>,
    #[serde(default)]
    optional_dependencies: BTreeMap<String, String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawBunMeta {
    #[serde(default)]
    dependencies: BTreeMap<String, String>,
    #[serde(default)]
    optional_dependencies: BTreeMap<String, String>,
    #[serde(default)]
    bin: BTreeMap<String, String>,
}

#[derive(Debug, Default)]
struct BunEntry {
    ident: String,
    meta: RawBunMeta,
    integrity: Option<String>,
}

impl BunEntry {
    fn from_array(key: &str, values: &[Value]) -> std::result::Result<Self, String> {
        let ident = values
            .first()
            .and_then(Value::as_str)
            .ok_or_else(|| format!("bun.lock package `{key}` is missing its ident string"))?
            .to_string();

        let mut meta = RawBunMeta::default();
        let mut integrity = None;

        for value in values.iter().skip(1) {
            match value {
                Value::Object(_) => {
                    meta = serde_json::from_value(value.clone()).unwrap_or_default();
                }
                Value::String(value) if is_integrity_hash(value) => {
                    integrity = Some(value.clone());
                }
                _ => {}
            }
        }

        Ok(Self {
            ident,
            meta,
            integrity,
        })
    }
}

fn decode_entries(
    path: &Path,
    packages: &BTreeMap<String, Vec<Value>>,
) -> Result<BTreeMap<String, BunEntry>> {
    let mut entries = BTreeMap::new();

    for (key, values) in packages {
        let entry = BunEntry::from_array(key, values).map_err(|reason| SnpmError::Lockfile {
            path: path.to_path_buf(),
            reason,
        })?;
        entries.insert(key.clone(), entry);
    }

    Ok(entries)
}

fn build_key_info(
    path: &Path,
    entries: &BTreeMap<String, BunEntry>,
) -> Result<BTreeMap<String, (String, String)>> {
    let mut key_info = BTreeMap::new();

    for (key, entry) in entries {
        let (name, version) = split_ident(&entry.ident).ok_or_else(|| SnpmError::Lockfile {
            path: path.to_path_buf(),
            reason: format!(
                "bun.lock package `{key}` has an unsupported ident `{}`",
                entry.ident
            ),
        })?;
        key_info.insert(key.clone(), (name, version));
    }

    Ok(key_info)
}

fn collect_workspace_names(workspaces: &BTreeMap<String, RawBunWorkspace>) -> BTreeSet<String> {
    workspaces
        .values()
        .filter_map(|workspace| workspace.name.clone())
        .collect()
}

fn build_packages(
    path: &Path,
    config: &SnpmConfig,
    entries: &BTreeMap<String, BunEntry>,
    key_info: &BTreeMap<String, (String, String)>,
) -> Result<BTreeMap<String, LockPackage>> {
    let mut packages = BTreeMap::new();

    for (key, entry) in entries {
        let (name, version) = key_info.get(key).ok_or_else(|| SnpmError::Lockfile {
            path: path.to_path_buf(),
            reason: format!("bun.lock package `{key}` disappeared during import"),
        })?;
        let lock_key = package_key(name, version);

        let mut dependencies = BTreeMap::new();
        for dep_name in entry.meta.dependencies.keys() {
            let dep_key = resolve_nested_bun(key, dep_name, key_info)
                .and_then(|target_key| {
                    key_info
                        .get(&target_key)
                        .map(|(target_name, target_version)| package_key(target_name, target_version))
                })
                .ok_or_else(|| SnpmError::Lockfile {
                    path: path.to_path_buf(),
                    reason: format!(
                        "bun.lock dependency `{dep_name}` from `{key}` could not be resolved from the imported lockfile"
                    ),
                })?;

            dependencies.insert(dep_name.clone(), dep_key);
        }

        for dep_name in entry.meta.optional_dependencies.keys() {
            if let Some(dep_key) =
                resolve_nested_bun(key, dep_name, key_info).and_then(|target_key| {
                    key_info
                        .get(&target_key)
                        .map(|(target_name, target_version)| {
                            package_key(target_name, target_version)
                        })
                })
            {
                dependencies.insert(dep_name.clone(), dep_key);
            }
        }

        let lock_package = LockPackage {
            name: name.clone(),
            version: version.clone(),
            tarball: derive_registry_tarball(config, name, version).unwrap_or_default(),
            integrity: entry.integrity.clone(),
            dependencies,
            bundled_dependencies: None,
            has_bin: !entry.meta.bin.is_empty(),
            bin: (!entry.meta.bin.is_empty()).then(|| BinField::Map(entry.meta.bin.clone())),
        };

        if let Some(existing) = packages.get(&lock_key) {
            if existing != &lock_package {
                return Err(SnpmError::Lockfile {
                    path: path.to_path_buf(),
                    reason: format!(
                        "bun.lock contains multiple package variants that collapse to the same snpm package key `{lock_key}`"
                    ),
                });
            }
            continue;
        }

        packages.insert(lock_key, lock_package);
    }

    Ok(packages)
}

fn build_root(
    path: &Path,
    workspaces: &BTreeMap<String, RawBunWorkspace>,
    key_info: &BTreeMap<String, (String, String)>,
    workspace_names: &BTreeSet<String>,
) -> Result<LockRoot> {
    let mut dependencies = BTreeMap::new();

    for (workspace_path, workspace) in workspaces {
        insert_root_block(
            path,
            workspace_path,
            &workspace.dependencies,
            false,
            key_info,
            workspace_names,
            &mut dependencies,
        )?;
        insert_root_block(
            path,
            workspace_path,
            &workspace.dev_dependencies,
            false,
            key_info,
            workspace_names,
            &mut dependencies,
        )?;
        insert_root_block(
            path,
            workspace_path,
            &workspace.optional_dependencies,
            true,
            key_info,
            workspace_names,
            &mut dependencies,
        )?;
    }

    Ok(LockRoot { dependencies })
}

fn insert_root_block(
    path: &Path,
    workspace_path: &str,
    block: &BTreeMap<String, String>,
    optional: bool,
    key_info: &BTreeMap<String, (String, String)>,
    workspace_names: &BTreeSet<String>,
    root: &mut BTreeMap<String, LockRootDependency>,
) -> Result<()> {
    for (dep_name, requested) in block {
        let resolved = resolve_workspace_dep(workspace_path, dep_name, key_info);

        let incoming = if let Some(dep_key) = resolved.as_deref() {
            if optional {
                build_optional_root_dependency(dep_name, requested, Some(dep_key))
            } else {
                build_required_root_dependency(path, dep_name, requested, dep_key)?
            }
        } else if workspace_names.contains(dep_name) {
            continue;
        } else if optional {
            build_optional_root_dependency(dep_name, requested, None)
        } else {
            return Err(SnpmError::Lockfile {
                path: path.to_path_buf(),
                reason: format!(
                    "bun.lock workspace `{}` dependency `{dep_name}` could not be resolved from the imported lockfile",
                    display_workspace_path(workspace_path)
                ),
            });
        };

        merge_root_dependency(path, workspace_path, dep_name, incoming, root)?;
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
    workspace_path: &str,
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
            workspace_path,
            dep_name,
            &existing.requested,
            &incoming.requested,
        ));
    }

    match (&existing.package, &incoming.package) {
        (Some(left), Some(right)) if left != right => {
            return Err(root_conflict_error(
                path,
                workspace_path,
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
                workspace_path,
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
    workspace_path: &str,
    dep_name: &str,
    left: &str,
    right: &str,
) -> SnpmError {
    SnpmError::Lockfile {
        path: path.to_path_buf(),
        reason: format!(
            "bun.lock workspace `{}` declares dependency `{dep_name}` with conflicting values `{left}` and `{right}`",
            display_workspace_path(workspace_path)
        ),
    }
}

fn resolve_nested_bun(
    package_key_path: &str,
    dep_name: &str,
    key_info: &BTreeMap<String, (String, String)>,
) -> Option<String> {
    let mut base = package_key_path.to_string();

    loop {
        let candidate = if base.is_empty() {
            dep_name.to_string()
        } else {
            format!("{base}/{dep_name}")
        };
        if key_info.contains_key(&candidate) {
            return Some(candidate);
        }

        if base.is_empty() {
            return None;
        }

        if let Some(index) = base.rfind('/') {
            let tail_start = base[..index].rfind('/').map(|value| value + 1).unwrap_or(0);
            if base[tail_start..index].starts_with('@') {
                base.truncate(tail_start.saturating_sub(1));
            } else {
                base.truncate(index);
            }
        } else {
            base.clear();
        }
    }
}

fn resolve_workspace_dep(
    workspace_path: &str,
    dep_name: &str,
    key_info: &BTreeMap<String, (String, String)>,
) -> Option<String> {
    if !workspace_path.is_empty() {
        let workspace_scoped = format!("{workspace_path}/{dep_name}");
        if let Some((name, version)) = key_info.get(&workspace_scoped) {
            return Some(package_key(name, version));
        }
    }

    key_info
        .get(dep_name)
        .map(|(name, version)| package_key(name, version))
}

fn split_ident(ident: &str) -> Option<(String, String)> {
    if let Some(rest) = ident.strip_prefix('@') {
        let slash = rest.find('/')?;
        let after_slash = &rest[slash + 1..];
        let at = after_slash.find('@')?;
        let name = format!("@{}", &rest[..slash + 1 + at]);
        let version = after_slash[at + 1..].to_string();
        Some((name, version))
    } else {
        let at = ident.find('@')?;
        Some((ident[..at].to_string(), ident[at + 1..].to_string()))
    }
}

fn is_integrity_hash(value: &str) -> bool {
    let Some((algorithm, body)) = value.split_once('-') else {
        return false;
    };

    let expected_length = match algorithm {
        "sha512" => 88,
        "sha384" => 64,
        "sha256" => 44,
        "sha1" => 28,
        "md5" => 24,
        _ => return false,
    };

    if body.len() != expected_length {
        return false;
    }

    body.bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'+' || byte == b'/' || byte == b'=')
}

fn strip_jsonc(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut index = 0;
    let mut in_string = false;
    let mut escaped = false;

    while index < bytes.len() {
        let byte = bytes[index];

        if in_string {
            output.push(byte as char);
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            index += 1;
            continue;
        }

        if byte == b'/' && index + 1 < bytes.len() && bytes[index + 1] == b'/' {
            while index < bytes.len() && bytes[index] != b'\n' {
                output.push(' ');
                index += 1;
            }
            continue;
        }

        if byte == b'/' && index + 1 < bytes.len() && bytes[index + 1] == b'*' {
            output.push(' ');
            output.push(' ');
            index += 2;
            while index + 1 < bytes.len() && !(bytes[index] == b'*' && bytes[index + 1] == b'/') {
                output.push(if bytes[index] == b'\n' { '\n' } else { ' ' });
                index += 1;
            }

            if index + 1 < bytes.len() {
                output.push(' ');
                output.push(' ');
                index += 2;
            } else {
                while index < bytes.len() {
                    output.push(if bytes[index] == b'\n' { '\n' } else { ' ' });
                    index += 1;
                }
            }
            continue;
        }

        if byte == b',' {
            let mut lookahead = index + 1;
            while lookahead < bytes.len() && (bytes[lookahead] as char).is_whitespace() {
                lookahead += 1;
            }

            if lookahead < bytes.len() && (bytes[lookahead] == b'}' || bytes[lookahead] == b']') {
                output.push(' ');
                index += 1;
                continue;
            }
        }

        if byte == b'"' {
            in_string = true;
        }

        output.push(byte as char);
        index += 1;
    }

    output
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

fn display_workspace_path(workspace_path: &str) -> &str {
    if workspace_path.is_empty() {
        "."
    } else {
        workspace_path
    }
}

#[cfg(test)]
mod tests {
    use super::{read, split_ident, strip_jsonc};
    use crate::config::{AuthScheme, HoistingMode, LinkBackend, SnpmConfig};

    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;

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
    fn imports_simple_bun_lockfile() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bun.lock");
        fs::write(
            &path,
            r#"{
  "lockfileVersion": 1,
  "workspaces": {
    "": {
      "name": "import-test",
      "dependencies": {
        "@sindresorhus/is": "5.6.0",
        "is-odd": "3.0.1",
      },
      "devDependencies": {
        "kind-of": "6.0.3",
      },
    },
  },
  "packages": {
    "@sindresorhus/is": ["@sindresorhus/is@5.6.0", "", {}, "sha512-TV7t8GKYaJWsn00tFDqBw8+Uqmr8A0fRU1tvTQhyZzGv0sJCGRQL3JGMI3ucuKo3XIZdUP+Lx7/gh2t3lewy7g=="],
    "is-number": ["is-number@6.0.0", "", {}, "sha512-Wu1VHeILBK8KAWJUAiSZQX94GmOE45Rg6/538fKwiloUu21KncEkYGPqob2oSZ5mUT73vLGrHQjKw3KMPwfDzg=="],
    "is-odd": ["is-odd@3.0.1", "", { "dependencies": { "is-number": "^6.0.0" } }, "sha512-CQpnWPrDwmP1+SMHXZhtLtJv90yiyVfluGsX5iNCVkrhQtU3TQHsUWPG9wkdk9Lgd5yNpAg9jQEo90CBaXgWMA=="],
    "kind-of": ["kind-of@6.0.3", "", {}, "sha512-dcS1ul+9tmeD95T+x28/ehLgd9mENa3LsvDTtzm3vyBEO7RPptvAD+t44WVXaUjTBRcrpFeFlC8WCruUR456hw=="],
  }
}"#,
        )
        .unwrap();

        let lockfile = read(&path, &test_config()).unwrap();

        assert_eq!(
            lockfile.root.dependencies["@sindresorhus/is"]
                .version
                .as_deref(),
            Some("5.6.0")
        );
        assert_eq!(
            lockfile.packages["is-odd@3.0.1"].dependencies["is-number"],
            "is-number@6.0.0"
        );
        assert!(lockfile.packages["kind-of@6.0.3"].has_bin == false);
    }

    #[test]
    fn imports_nested_bun_overrides() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bun.lock");
        fs::write(
            &path,
            r#"{
  "lockfileVersion": 1,
  "workspaces": {
    "": {
      "name": "nested-test",
      "dependencies": {
        "foo": "^1.0.0",
        "bar": "^2.0.0"
      }
    }
  },
  "packages": {
    "foo": ["foo@1.0.0", "", { "dependencies": { "bar": "^1.0.0" } }, "sha512-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"],
    "bar": ["bar@2.0.0", "", {}, "sha512-bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"],
    "foo/bar": ["bar@1.0.0", "", {}, "sha512-cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"]
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
    fn imports_multi_workspace_bun_lockfile() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bun.lock");
        fs::write(
            &path,
            r#"{
  "lockfileVersion": 1,
  "workspaces": {
    "": {
      "name": "root",
      "dependencies": { "foo": "^1.0.0" }
    },
    "packages/app": {
      "name": "app",
      "dependencies": { "bar": "^3.0.0" }
    }
  },
  "packages": {
    "foo": ["foo@1.2.3", "", {}, "sha512-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"],
    "bar": ["bar@3.1.0", "", {}, "sha512-bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"]
  }
}"#,
        )
        .unwrap();

        let lockfile = read(&path, &test_config()).unwrap();

        assert_eq!(
            lockfile.root.dependencies["foo"].version.as_deref(),
            Some("1.2.3")
        );
        assert_eq!(
            lockfile.root.dependencies["bar"].version.as_deref(),
            Some("3.1.0")
        );
        assert_eq!(lockfile.packages.len(), 2);
    }

    #[test]
    fn skips_local_workspace_direct_dependencies() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bun.lock");
        fs::write(
            &path,
            r#"{
  "lockfileVersion": 1,
  "workspaces": {
    "": { "name": "root" },
    "packages/app": {
      "name": "app",
      "dependencies": {
        "lib": "^1.0.0",
        "left-pad": "^1.3.0"
      }
    },
    "packages/lib": {
      "name": "lib"
    }
  },
  "packages": {
    "left-pad": ["left-pad@1.3.0", "", {}, "sha512-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"]
  }
}"#,
        )
        .unwrap();

        let lockfile = read(&path, &test_config()).unwrap();

        assert!(lockfile.root.dependencies.contains_key("left-pad"));
        assert!(!lockfile.root.dependencies.contains_key("lib"));
    }

    #[test]
    fn rejects_unsupported_bun_lockfile_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bun.lock");
        fs::write(
            &path,
            r#"{
  "lockfileVersion": 2,
  "workspaces": {},
  "packages": {}
}"#,
        )
        .unwrap();

        let error = read(&path, &test_config()).unwrap_err();
        assert!(matches!(
            error,
            crate::SnpmError::Lockfile { reason, .. } if reason.contains("lockfileVersion 2")
        ));
    }

    #[test]
    fn split_ident_handles_scoped_packages() {
        assert_eq!(
            split_ident("@scope/pkg@1.0.0"),
            Some(("@scope/pkg".to_string(), "1.0.0".to_string()))
        );
        assert_eq!(
            split_ident("foo@1.2.3"),
            Some(("foo".to_string(), "1.2.3".to_string()))
        );
    }

    #[test]
    fn strip_jsonc_removes_comments_and_trailing_commas() {
        let input = "{\n  // comment\n  \"a\": 1,\n}\n";
        let output = strip_jsonc(input);
        assert_eq!(output.len(), input.len());
        let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(parsed["a"], 1);
    }
}
