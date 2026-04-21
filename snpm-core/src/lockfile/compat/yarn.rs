use super::super::types::{LockPackage, LockRoot, LockRootDependency, Lockfile};
use crate::operations::install::{
    RootSpecSet, apply_specs, build_project_root_specs, collect_workspace_root_specs,
};
use crate::protocols::encode_package_name;
use crate::workspace::CatalogConfig;
use crate::{Project, Result, SnpmConfig, SnpmError, Workspace};

use serde_yaml::Value as YamlValue;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

pub(super) fn read(path: &Path, config: &SnpmConfig) -> Result<Lockfile> {
    let content = fs::read_to_string(path).map_err(|source| SnpmError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;
    let root_specs = load_manifest_root_specs(path)?;

    if is_berry(&content) {
        parse_berry_str(path, &content, config, &root_specs)
    } else {
        parse_classic_str(path, &content, config, &root_specs)
    }
}

#[derive(Debug)]
struct ClassicBlock {
    specs: Vec<String>,
    fields: BTreeMap<String, String>,
    dependencies: BTreeMap<String, String>,
}

#[derive(Debug)]
struct ClassicIdentity {
    package_name: String,
}

#[derive(Debug)]
struct ClassicPackageSeed {
    dep_path: String,
    name: String,
    version: String,
    tarball: String,
    integrity: Option<String>,
    declared_dependencies: BTreeMap<String, String>,
}

#[derive(Debug)]
struct BerryPackageSeed {
    dep_path: String,
    name: String,
    version: String,
    tarball: String,
    has_bin: bool,
    declared_dependencies: BTreeMap<String, String>,
}

fn parse_classic_str(
    path: &Path,
    content: &str,
    config: &SnpmConfig,
    root_specs: &RootSpecSet,
) -> Result<Lockfile> {
    let blocks = tokenize_classic_blocks(content).map_err(|reason| SnpmError::Lockfile {
        path: path.to_path_buf(),
        reason,
    })?;
    let project_root = path.parent().unwrap_or(Path::new("."));

    let mut spec_to_dep_path = BTreeMap::new();
    let mut packages = BTreeMap::new();
    let mut package_seeds = Vec::new();
    let mut seen_dep_paths = std::collections::BTreeSet::new();

    for block in &blocks {
        let version = block
            .fields
            .get("version")
            .cloned()
            .ok_or_else(|| SnpmError::Lockfile {
                path: path.to_path_buf(),
                reason: format!("yarn.lock block {:?} is missing a version", block.specs),
            })?;

        let identity =
            parse_classic_identity(&block.specs[0]).ok_or_else(|| SnpmError::Lockfile {
                path: path.to_path_buf(),
                reason: format!(
                    "could not parse package name from yarn.lock spec `{}`",
                    block.specs[0]
                ),
            })?;
        let dep_path = format!("{}@{}", identity.package_name, version);

        for spec in &block.specs {
            let block_identity =
                parse_classic_identity(spec).ok_or_else(|| SnpmError::Lockfile {
                    path: path.to_path_buf(),
                    reason: format!("could not parse package name from yarn.lock spec `{spec}`"),
                })?;
            if block_identity.package_name != identity.package_name {
                return Err(SnpmError::Lockfile {
                    path: path.to_path_buf(),
                    reason: format!(
                        "yarn.lock block {:?} resolves multiple package names",
                        block.specs
                    ),
                });
            }

            spec_to_dep_path.insert(spec.clone(), dep_path.clone());
        }

        if !seen_dep_paths.insert(dep_path.clone()) {
            continue;
        }

        package_seeds.push(ClassicPackageSeed {
            dep_path,
            name: identity.package_name.clone(),
            version: version.clone(),
            tarball: classic_tarball(
                project_root,
                config,
                &identity.package_name,
                &version,
                block.fields.get("resolved").map(String::as_str),
            )
            .unwrap_or_default(),
            integrity: block.fields.get("integrity").cloned(),
            declared_dependencies: block.dependencies.clone(),
        });
    }

    for seed in package_seeds {
        packages.insert(
            seed.dep_path.clone(),
            LockPackage {
                name: seed.name,
                version: seed.version,
                tarball: seed.tarball,
                integrity: seed.integrity,
                dependencies: resolve_dependencies(&seed.declared_dependencies, &spec_to_dep_path),
                bundled_dependencies: None,
                has_bin: false,
                bin: None,
            },
        );
    }

    let root = build_root_from_specs(path, root_specs, &spec_to_dep_path, classic_candidates)?;

    Ok(Lockfile {
        version: 1,
        root,
        packages,
    })
}

fn parse_berry_str(
    path: &Path,
    content: &str,
    config: &SnpmConfig,
    root_specs: &RootSpecSet,
) -> Result<Lockfile> {
    let doc: YamlValue = serde_yaml::from_str(content).map_err(|error| SnpmError::Lockfile {
        path: path.to_path_buf(),
        reason: format!("failed to parse yarn.lock: {error}"),
    })?;
    let map = doc.as_mapping().ok_or_else(|| SnpmError::Lockfile {
        path: path.to_path_buf(),
        reason: "yarn berry lockfile root must be a mapping".into(),
    })?;

    let metadata_version = map
        .get(YamlValue::String("__metadata".to_string()))
        .and_then(YamlValue::as_mapping)
        .and_then(|metadata| metadata.get(YamlValue::String("version".to_string())))
        .and_then(yaml_scalar_as_string)
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    if metadata_version < 3 {
        return Err(SnpmError::Lockfile {
            path: path.to_path_buf(),
            reason: format!(
                "yarn berry lockfile has unexpected __metadata.version: {metadata_version} (expected >= 3)"
            ),
        });
    }

    let project_root = path.parent().unwrap_or(Path::new("."));
    let mut spec_to_dep_path = BTreeMap::new();
    let mut packages = BTreeMap::new();
    let mut package_seeds = Vec::new();
    let mut seen_dep_paths = std::collections::BTreeSet::new();

    for (key, value) in map {
        let Some(key_str) = key.as_str() else {
            continue;
        };
        if key_str.starts_with("__") {
            continue;
        }

        let block = value.as_mapping().ok_or_else(|| SnpmError::Lockfile {
            path: path.to_path_buf(),
            reason: format!("yarn berry block `{key_str}` is not a mapping"),
        })?;
        let specs = split_berry_header(key_str);
        if specs.is_empty() {
            continue;
        }

        let version = block
            .get(YamlValue::String("version".to_string()))
            .and_then(yaml_scalar_as_string)
            .ok_or_else(|| SnpmError::Lockfile {
                path: path.to_path_buf(),
                reason: format!("yarn berry block `{key_str}` is missing a version"),
            })?;

        let resolution = block
            .get(YamlValue::String("resolution".to_string()))
            .and_then(YamlValue::as_str)
            .ok_or_else(|| SnpmError::Lockfile {
                path: path.to_path_buf(),
                reason: format!("yarn berry block `{key_str}` is missing a resolution"),
            })?;
        let (resolved_name, protocol, body) =
            parse_berry_spec(resolution).ok_or_else(|| SnpmError::Lockfile {
                path: path.to_path_buf(),
                reason: format!(
                    "yarn berry block `{key_str}` has malformed resolution `{resolution}`"
                ),
            })?;

        if protocol == "workspace" {
            continue;
        }

        let dep_path = format!("{resolved_name}@{version}");
        for spec in &specs {
            spec_to_dep_path.insert(spec.clone(), dep_path.clone());
        }

        if !seen_dep_paths.insert(dep_path.clone()) {
            continue;
        }

        let tarball = match protocol {
            "npm" => derive_registry_tarball(config, resolved_name, &version),
            "file" | "link" => local_file_url(project_root, strip_hash_fragment(body)),
            unsupported => {
                return Err(SnpmError::Lockfile {
                    path: path.to_path_buf(),
                    reason: format!("yarn berry protocol `{unsupported}` is not supported yet"),
                });
            }
        }
        .unwrap_or_default();

        let declared_dependencies = collect_dep_map(block, "dependencies")
            .into_iter()
            .chain(collect_dep_map(block, "optionalDependencies"))
            .collect();

        package_seeds.push(BerryPackageSeed {
            dep_path,
            name: resolved_name.to_string(),
            version: version.clone(),
            tarball,
            has_bin: berry_has_bin(block),
            declared_dependencies,
        });
    }

    for seed in package_seeds {
        packages.insert(
            seed.dep_path.clone(),
            LockPackage {
                name: seed.name,
                version: seed.version,
                tarball: seed.tarball,
                integrity: None,
                dependencies: resolve_dependencies_with(
                    &seed.declared_dependencies,
                    &spec_to_dep_path,
                    berry_candidates,
                ),
                bundled_dependencies: None,
                has_bin: seed.has_bin,
                bin: None,
            },
        );
    }

    let root = build_root_from_specs(path, root_specs, &spec_to_dep_path, berry_candidates)?;

    Ok(Lockfile {
        version: 1,
        root,
        packages,
    })
}

fn load_manifest_root_specs(lockfile_path: &Path) -> Result<RootSpecSet> {
    let lockfile_root = lockfile_path.parent().unwrap_or(Path::new("."));

    if let Some(workspace) = Workspace::discover(lockfile_root)?
        && workspace.root == lockfile_root
    {
        return collect_workspace_root_specs(&workspace, true);
    }

    let project = Project::from_manifest_path(lockfile_root.join("package.json"))?;
    let catalog = CatalogConfig::load(&project.root)?;

    let mut local_deps = std::collections::BTreeSet::new();
    let mut local_dev_deps = std::collections::BTreeSet::new();
    let mut local_optional_deps = std::collections::BTreeSet::new();

    let dependencies = apply_specs(
        &project.manifest.dependencies,
        None,
        catalog.as_ref(),
        &mut local_deps,
        None,
    )?;
    let development_dependencies = apply_specs(
        &project.manifest.dev_dependencies,
        None,
        catalog.as_ref(),
        &mut local_dev_deps,
        None,
    )?;
    let optional_dependencies = apply_specs(
        &project.manifest.optional_dependencies,
        None,
        catalog.as_ref(),
        &mut local_optional_deps,
        None,
    )?;

    Ok(build_project_root_specs(
        &dependencies,
        &development_dependencies,
        &optional_dependencies,
        true,
    ))
}

fn build_root_from_specs<F>(
    path: &Path,
    root_specs: &RootSpecSet,
    spec_to_dep_path: &BTreeMap<String, String>,
    candidate_builder: F,
) -> Result<LockRoot>
where
    F: Fn(&str, &str) -> Vec<String>,
{
    let mut dependencies = BTreeMap::new();

    for (name, requested) in &root_specs.required {
        let dep_path = resolve_spec_candidates(&candidate_builder(name, requested), spec_to_dep_path)
            .ok_or_else(|| SnpmError::Lockfile {
                path: path.to_path_buf(),
                reason: format!(
                    "yarn.lock root dependency `{name}` -> `{requested}` could not be resolved from the imported lockfile"
                ),
            })?;

        let (resolved_name, version) =
            split_dep_path(&dep_path).ok_or_else(|| SnpmError::Lockfile {
                path: path.to_path_buf(),
                reason: format!("unsupported resolved dependency key `{dep_path}`"),
            })?;

        dependencies.insert(
            name.clone(),
            LockRootDependency {
                requested: requested.clone(),
                package: (resolved_name != *name).then_some(resolved_name),
                version: Some(version),
                optional: false,
            },
        );
    }

    for (name, requested) in &root_specs.optional {
        let resolved =
            resolve_spec_candidates(&candidate_builder(name, requested), spec_to_dep_path);
        let dependency = if let Some(dep_path) = resolved {
            let (resolved_name, version) =
                split_dep_path(&dep_path).ok_or_else(|| SnpmError::Lockfile {
                    path: path.to_path_buf(),
                    reason: format!("unsupported resolved dependency key `{dep_path}`"),
                })?;

            LockRootDependency {
                requested: requested.clone(),
                package: (resolved_name != *name).then_some(resolved_name),
                version: Some(version),
                optional: true,
            }
        } else {
            LockRootDependency {
                requested: requested.clone(),
                package: None,
                version: None,
                optional: true,
            }
        };

        dependencies.insert(name.clone(), dependency);
    }

    Ok(LockRoot { dependencies })
}

fn resolve_spec_candidates(
    candidates: &[String],
    spec_to_dep_path: &BTreeMap<String, String>,
) -> Option<String> {
    candidates
        .iter()
        .find_map(|candidate| spec_to_dep_path.get(candidate).cloned())
}

fn resolve_dependencies(
    declared_dependencies: &BTreeMap<String, String>,
    spec_to_dep_path: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    resolve_dependencies_with(declared_dependencies, spec_to_dep_path, classic_candidates)
}

fn resolve_dependencies_with<F>(
    declared_dependencies: &BTreeMap<String, String>,
    spec_to_dep_path: &BTreeMap<String, String>,
    candidate_builder: F,
) -> BTreeMap<String, String>
where
    F: Fn(&str, &str) -> Vec<String>,
{
    let mut dependencies = BTreeMap::new();

    for (name, range) in declared_dependencies {
        if let Some(dep_path) =
            resolve_spec_candidates(&candidate_builder(name, range), spec_to_dep_path)
        {
            dependencies.insert(name.clone(), dep_path);
        }
    }

    dependencies
}

fn tokenize_classic_blocks(content: &str) -> std::result::Result<Vec<ClassicBlock>, String> {
    let mut blocks = Vec::new();
    let mut current = None;
    let mut in_dependencies = false;

    for (index, raw_line) in content.lines().enumerate() {
        let line_number = index + 1;
        let line = raw_line.trim_end();

        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }

        let indent = line.len() - line.trim_start().len();
        if indent == 0 {
            if let Some(block) = current.take() {
                blocks.push(block);
            }

            in_dependencies = false;
            if !line.ends_with(':') {
                return Err(format!(
                    "line {line_number}: expected block header ending in `:`"
                ));
            }

            let specs = parse_classic_header_specs(line.trim_end_matches(':').trim())?;
            current = Some(ClassicBlock {
                specs,
                fields: BTreeMap::new(),
                dependencies: BTreeMap::new(),
            });
            continue;
        }

        let Some(block) = current.as_mut() else {
            return Err(format!(
                "line {line_number}: encountered indented content before any block header"
            ));
        };

        if indent == 2 {
            in_dependencies = false;
            let body = line.trim_start();
            if body.ends_with(':') {
                in_dependencies = body.trim_end_matches(':').trim() == "dependencies";
                continue;
            }

            let (key, value) = split_classic_key_value(body)
                .ok_or_else(|| format!("line {line_number}: could not parse `{body}`"))?;
            block.fields.insert(key, value);
            continue;
        }

        if indent >= 4 && in_dependencies {
            let body = line.trim_start();
            let (name, value) = split_classic_key_value(body).ok_or_else(|| {
                format!("line {line_number}: could not parse dependency `{body}`")
            })?;
            block.dependencies.insert(name, value);
        }
    }

    if let Some(block) = current.take() {
        blocks.push(block);
    }

    Ok(blocks)
}

fn parse_classic_header_specs(header: &str) -> std::result::Result<Vec<String>, String> {
    let mut specs = Vec::new();

    for raw_spec in header.split(',') {
        let spec = raw_spec.trim();
        let spec = if (spec.starts_with('"') && spec.ends_with('"') && spec.len() >= 2)
            || (spec.starts_with('\'') && spec.ends_with('\'') && spec.len() >= 2)
        {
            &spec[1..spec.len() - 1]
        } else {
            spec
        };

        if spec.is_empty() {
            return Err(format!("empty spec in header `{header}`"));
        }

        specs.push(spec.to_string());
    }

    if specs.is_empty() {
        return Err(format!("no specs parsed from header `{header}`"));
    }

    Ok(specs)
}

fn split_classic_key_value(line: &str) -> Option<(String, String)> {
    let (key, value) = line.split_once(char::is_whitespace)?;
    let value = value.trim();
    let value = if (value.starts_with('"') && value.ends_with('"') && value.len() >= 2)
        || (value.starts_with('\'') && value.ends_with('\'') && value.len() >= 2)
    {
        &value[1..value.len() - 1]
    } else {
        value
    };

    Some((key.to_string(), value.to_string()))
}

fn parse_classic_identity(spec: &str) -> Option<ClassicIdentity> {
    let (outer_name, range) = split_spec_name_and_range(spec)?;
    if let Some(inner) = range.strip_prefix("npm:") {
        let (package_name, _) = split_spec_name_and_range(inner)?;
        return Some(ClassicIdentity { package_name });
    }

    Some(ClassicIdentity {
        package_name: outer_name,
    })
}

fn split_spec_name_and_range(spec: &str) -> Option<(String, String)> {
    if let Some(rest) = spec.strip_prefix('@') {
        let slash = rest.find('/')?;
        let after_slash = &rest[slash + 1..];
        let at = after_slash.find('@')?;
        let name_end = slash + 1 + at;
        let name = format!("@{}", &rest[..name_end]);
        let range = after_slash[at + 1..].to_string();
        Some((name, range))
    } else {
        let at = spec.find('@')?;
        Some((spec[..at].to_string(), spec[at + 1..].to_string()))
    }
}

fn classic_candidates(name: &str, range: &str) -> Vec<String> {
    vec![format!("{name}@{range}")]
}

fn classic_tarball(
    project_root: &Path,
    config: &SnpmConfig,
    name: &str,
    version: &str,
    resolved: Option<&str>,
) -> Option<String> {
    if let Some(resolved) = resolved {
        if resolved.starts_with("http://") || resolved.starts_with("https://") {
            return Some(strip_hash_fragment(resolved).to_string());
        }

        if resolved.starts_with("file:") || resolved.starts_with("link:") {
            return local_file_url(
                project_root,
                strip_hash_fragment(
                    resolved
                        .trim_start_matches("file:")
                        .trim_start_matches("link:"),
                ),
            );
        }
    }

    derive_registry_tarball(config, name, version)
}

fn is_berry(content: &str) -> bool {
    content
        .lines()
        .any(|line| line.trim_start().starts_with("__metadata:"))
}

fn split_berry_header(header: &str) -> Vec<String> {
    header
        .split(", ")
        .map(|value| value.trim().trim_matches('"').to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

fn parse_berry_spec(spec: &str) -> Option<(&str, &str, &str)> {
    let (name, after_at) = if let Some(rest) = spec.strip_prefix('@') {
        let slash = rest.find('/')?;
        let after_slash = &rest[slash + 1..];
        let at = after_slash.find('@')?;
        let full_name_len = 1 + slash + 1 + at;
        (&spec[..full_name_len], &spec[full_name_len + 1..])
    } else {
        let at = spec.find('@')?;
        (&spec[..at], &spec[at + 1..])
    };

    let colon = after_at.find(':')?;
    let protocol = &after_at[..colon];
    let body = &after_at[colon + 1..];
    Some((name, protocol, body))
}

fn berry_candidates(name: &str, range: &str) -> Vec<String> {
    let mut candidates = vec![format!("{name}@{range}")];
    if !range_has_protocol(range) {
        candidates.push(format!("{name}@npm:{range}"));
    }
    candidates
}

fn range_has_protocol(range: &str) -> bool {
    let Some(colon) = range.find(':') else {
        return false;
    };

    let head = &range[..colon];
    !head.is_empty()
        && head
            .chars()
            .all(|character| character.is_ascii_alphabetic() || character == '+')
}

fn yaml_scalar_as_string(value: &YamlValue) -> Option<String> {
    match value {
        YamlValue::String(value) => Some(value.clone()),
        YamlValue::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn collect_dep_map(block: &serde_yaml::Mapping, key: &str) -> BTreeMap<String, String> {
    block
        .get(YamlValue::String(key.to_string()))
        .and_then(YamlValue::as_mapping)
        .map(|mapping| {
            mapping
                .iter()
                .filter_map(|(key, value)| {
                    Some((key.as_str()?.to_string(), yaml_scalar_as_string(value)?))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn berry_has_bin(block: &serde_yaml::Mapping) -> bool {
    block
        .get(YamlValue::String("bin".to_string()))
        .and_then(YamlValue::as_mapping)
        .is_some_and(|mapping| !mapping.is_empty())
}

fn strip_hash_fragment(value: &str) -> &str {
    value.split_once('#').map(|(head, _)| head).unwrap_or(value)
}

fn local_file_url(project_root: &Path, relative_path: &str) -> Option<String> {
    let trimmed = relative_path.trim();
    if trimmed.is_empty() {
        return None;
    }

    let trimmed = trimmed.strip_prefix("file:").unwrap_or(trimmed);
    let trimmed = trimmed.strip_prefix("link:").unwrap_or(trimmed);
    Some(format!("file://{}", project_root.join(trimmed).display()))
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

fn split_dep_path(dep_path: &str) -> Option<(String, String)> {
    let index = dep_path.rfind('@')?;
    let (name, version) = dep_path.split_at(index);
    Some((
        name.to_string(),
        version.trim_start_matches('@').to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::{is_berry, parse_classic_identity, read, split_berry_header};
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
    fn imports_yarn_classic_lockfile() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("package.json"),
            r#"{
  "name": "import-test",
  "version": "1.0.0",
  "dependencies": {
    "is-odd": "^3.0.0"
  },
  "devDependencies": {
    "@sindresorhus/is": "^5.0.0"
  }
}"#,
        )
        .unwrap();
        let path = dir.path().join("yarn.lock");
        fs::write(
            &path,
            r#"# yarn lockfile v1

"@sindresorhus/is@^5.0.0":
  version "5.6.0"
  resolved "https://registry.yarnpkg.com/@sindresorhus/is/-/is-5.6.0.tgz#hash"
  integrity sha512-abc

"is-odd@^3.0.0":
  version "3.0.1"
  resolved "https://registry.yarnpkg.com/is-odd/-/is-odd-3.0.1.tgz#hash"
  integrity sha512-ghi
  dependencies:
    is-number "^6.0.0"

"is-number@^6.0.0":
  version "6.0.0"
  resolved "https://registry.yarnpkg.com/is-number/-/is-number-6.0.0.tgz#hash"
  integrity sha512-def
"#,
        )
        .unwrap();

        let lockfile = read(&path, &test_config()).unwrap();
        assert_eq!(
            lockfile.root.dependencies["is-odd"].version.as_deref(),
            Some("3.0.1")
        );
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
    }

    #[test]
    fn imports_yarn_classic_alias_root_dependency() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("package.json"),
            r#"{
  "name": "alias-test",
  "version": "1.0.0",
  "dependencies": {
    "h3-v2": "npm:h3@^2.0.0"
  }
}"#,
        )
        .unwrap();
        let path = dir.path().join("yarn.lock");
        fs::write(
            &path,
            r#"# yarn lockfile v1

"h3-v2@npm:h3@^2.0.0":
  version "2.0.1"
  resolved "https://registry.yarnpkg.com/h3/-/h3-2.0.1.tgz#hash"
"#,
        )
        .unwrap();

        let lockfile = read(&path, &test_config()).unwrap();
        let dependency = &lockfile.root.dependencies["h3-v2"];
        assert_eq!(dependency.package.as_deref(), Some("h3"));
        assert_eq!(dependency.version.as_deref(), Some("2.0.1"));
        assert!(lockfile.packages.contains_key("h3@2.0.1"));
    }

    #[test]
    fn imports_yarn_classic_multi_spec_header() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("package.json"),
            r#"{
  "name": "classic-multi-spec-test",
  "version": "1.0.0",
  "dependencies": {
    "foo": "~1.0.0"
  }
}"#,
        )
        .unwrap();
        let path = dir.path().join("yarn.lock");
        fs::write(
            &path,
            r#"# yarn lockfile v1

"foo@^1.0.0", "foo@~1.0.0":
  version "1.2.3"
  resolved "https://registry.yarnpkg.com/foo/-/foo-1.2.3.tgz#hash"
  dependencies:
    bar "^2.0.0"

"bar@^2.0.0":
  version "2.4.0"
  resolved "https://registry.yarnpkg.com/bar/-/bar-2.4.0.tgz#hash"
"#,
        )
        .unwrap();

        let lockfile = read(&path, &test_config()).unwrap();
        assert_eq!(
            lockfile.root.dependencies["foo"].version.as_deref(),
            Some("1.2.3")
        );
        assert_eq!(
            lockfile.packages["foo@1.2.3"].dependencies["bar"],
            "bar@2.4.0"
        );
    }

    #[test]
    fn imports_shared_yarn_classic_workspace_lockfile() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("package.json"),
            r#"{
  "private": true,
  "workspaces": ["packages/*"]
}"#,
        )
        .unwrap();
        fs::create_dir_all(dir.path().join("packages/foo")).unwrap();
        fs::write(
            dir.path().join("packages/foo/package.json"),
            r#"{
  "name": "foo",
  "version": "1.0.0",
  "dependencies": {
    "is-positive": "^1.0.0"
  }
}"#,
        )
        .unwrap();
        fs::create_dir_all(dir.path().join("packages/bar")).unwrap();
        fs::write(
            dir.path().join("packages/bar/package.json"),
            r#"{
  "name": "bar",
  "version": "1.0.0",
  "dependencies": {
    "is-negative": "^1.0.0"
  },
  "devDependencies": {
    "foo": "workspace:*"
  }
}"#,
        )
        .unwrap();
        let path = dir.path().join("yarn.lock");
        fs::write(
            &path,
            r#"# yarn lockfile v1

is-negative@^1.0.0:
  version "1.0.1"
  resolved "https://registry.yarnpkg.com/is-negative/-/is-negative-1.0.1.tgz#hash"

is-positive@^1.0.0:
  version "1.0.0"
  resolved "https://registry.yarnpkg.com/is-positive/-/is-positive-1.0.0.tgz#hash"
"#,
        )
        .unwrap();

        let lockfile = read(&path, &test_config()).unwrap();
        assert!(lockfile.root.dependencies.contains_key("is-negative"));
        assert!(lockfile.root.dependencies.contains_key("is-positive"));
        assert!(!lockfile.root.dependencies.contains_key("foo"));
    }

    #[test]
    fn imports_yarn_berry_npm_lockfile() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("package.json"),
            r#"{
  "name": "berry-test",
  "version": "1.0.0",
  "dependencies": {
    "foo": "^1.0.0"
  }
}"#,
        )
        .unwrap();
        let path = dir.path().join("yarn.lock");
        fs::write(
            &path,
            r#"__metadata:
  version: 8
  cacheKey: 10c0

"foo@npm:^1.0.0":
  version: 1.2.3
  resolution: "foo@npm:1.2.3"
  dependencies:
    bar: "npm:^2.0.0"

"bar@npm:^2.0.0":
  version: 2.5.0
  resolution: "bar@npm:2.5.0"
"#,
        )
        .unwrap();

        let lockfile = read(&path, &test_config()).unwrap();
        assert_eq!(
            lockfile.root.dependencies["foo"].version.as_deref(),
            Some("1.2.3")
        );
        assert_eq!(
            lockfile.packages["foo@1.2.3"].dependencies["bar"],
            "bar@2.5.0"
        );
    }

    #[test]
    fn imports_yarn_berry_multi_spec_header() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("package.json"),
            r#"{
  "name": "berry-multi-spec-test",
  "version": "1.0.0",
  "dependencies": {
    "minimatch": "^3.0.4"
  }
}"#,
        )
        .unwrap();
        let path = dir.path().join("yarn.lock");
        fs::write(
            &path,
            r#"__metadata:
  version: 8
  cacheKey: 10c0

"minimatch@npm:2 || 3, minimatch@npm:^3.0.4":
  version: 3.0.4
  resolution: "minimatch@npm:3.0.4"
  dependencies:
    brace-expansion: "npm:^1.1.7"

"brace-expansion@npm:^1.1.7":
  version: 1.1.11
  resolution: "brace-expansion@npm:1.1.11"
"#,
        )
        .unwrap();

        let lockfile = read(&path, &test_config()).unwrap();
        assert_eq!(
            lockfile.root.dependencies["minimatch"].version.as_deref(),
            Some("3.0.4")
        );
        assert_eq!(
            lockfile.packages["minimatch@3.0.4"].dependencies["brace-expansion"],
            "brace-expansion@1.1.11"
        );
    }

    #[test]
    fn imports_yarn_berry_bin_metadata() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("package.json"),
            r#"{
  "name": "berry-bin-test",
  "version": "1.0.0",
  "dependencies": {
    "rimraf": "2.5.1"
  }
}"#,
        )
        .unwrap();
        let path = dir.path().join("yarn.lock");
        fs::write(
            &path,
            r#"__metadata:
  version: 8
  cacheKey: 10c0

"rimraf@npm:2.5.1":
  version: 2.5.1
  resolution: "rimraf@npm:2.5.1"
  bin:
    rimraf: ./bin.js
"#,
        )
        .unwrap();

        let lockfile = read(&path, &test_config()).unwrap();
        assert!(lockfile.packages["rimraf@2.5.1"].has_bin);
    }

    #[test]
    fn imports_yarn_berry_workspace_lockfile() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("package.json"),
            r#"{
  "private": true,
  "workspaces": ["packages/*"]
}"#,
        )
        .unwrap();
        fs::create_dir_all(dir.path().join("packages/app")).unwrap();
        fs::write(
            dir.path().join("packages/app/package.json"),
            r#"{
  "name": "app",
  "version": "1.0.0",
  "dependencies": {
    "foo": "^1.0.0",
    "lib": "workspace:*"
  }
}"#,
        )
        .unwrap();
        fs::create_dir_all(dir.path().join("packages/lib")).unwrap();
        fs::write(
            dir.path().join("packages/lib/package.json"),
            r#"{
  "name": "lib",
  "version": "1.0.0"
}"#,
        )
        .unwrap();
        let path = dir.path().join("yarn.lock");
        fs::write(
            &path,
            r#"__metadata:
  version: 8
  cacheKey: 10c0

"app@workspace:packages/app":
  version: 0.0.0-use.local
  resolution: "app@workspace:packages/app"

"lib@workspace:packages/lib":
  version: 0.0.0-use.local
  resolution: "lib@workspace:packages/lib"

"foo@npm:^1.0.0":
  version: 1.2.3
  resolution: "foo@npm:1.2.3"
"#,
        )
        .unwrap();

        let lockfile = read(&path, &test_config()).unwrap();
        assert_eq!(lockfile.root.dependencies.len(), 1);
        assert!(lockfile.root.dependencies.contains_key("foo"));
        assert!(!lockfile.root.dependencies.contains_key("lib"));
    }

    #[test]
    fn rejects_unsupported_yarn_berry_protocol() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("package.json"),
            r#"{
  "name": "berry-test",
  "version": "1.0.0"
}"#,
        )
        .unwrap();
        let path = dir.path().join("yarn.lock");
        fs::write(
            &path,
            r#"__metadata:
  version: 8
  cacheKey: 10c0

"foo@git+ssh://git@github.com/user/repo.git":
  version: 1.0.0
  resolution: "foo@git+ssh://git@github.com/user/repo.git#abcdef"
"#,
        )
        .unwrap();

        let error = read(&path, &test_config()).unwrap_err();
        assert!(matches!(
            error,
            crate::SnpmError::Lockfile { reason, .. } if reason.contains("protocol `git+ssh`")
        ));
    }

    #[test]
    fn detects_berry_from_metadata() {
        assert!(is_berry("__metadata:\n  version: 8\n"));
        assert!(!is_berry("# yarn lockfile v1\n"));
    }

    #[test]
    fn parses_classic_alias_identity() {
        let identity = parse_classic_identity("h3-v2@npm:h3@^2.0.0").unwrap();
        assert_eq!(identity.package_name, "h3");
    }

    #[test]
    fn splits_berry_headers() {
        assert_eq!(
            split_berry_header("foo@npm:^1.0.0, foo@npm:^2.0.0"),
            vec!["foo@npm:^1.0.0", "foo@npm:^2.0.0"]
        );
    }
}
