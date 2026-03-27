use crate::operations::install::utils::ParsedSpec;
use crate::registry::RegistryProtocol;
use crate::resolve::ResolutionGraph;
use crate::workspace::CatalogConfig;
use crate::{Project, Result, SnpmError, Workspace};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use super::workspace::validate_workspace_spec;

#[derive(Debug, Clone)]
pub struct RootSpecSet {
    pub required: BTreeMap<String, String>,
    pub optional: BTreeMap<String, String>,
}

pub fn write_manifest(
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

        let mut spec = format!("^{}", dep.resolved.version);

        if let Some(workspace_reference) = workspace {
            if workspace_reference.config.catalog.contains_key(name) {
                spec = "catalog:".to_string();
            } else {
                for (catalog_name, entries) in workspace_reference.config.catalogs.iter() {
                    if entries.contains_key(name) {
                        spec = format!("catalog:{catalog_name}");
                        break;
                    }
                }
            }
        } else if let Some(catalog_config) = catalog {
            if catalog_config.catalog.contains_key(name) {
                spec = "catalog:".to_string();
            } else {
                for (catalog_name, entries) in catalog_config.catalogs.iter() {
                    if entries.contains_key(name) {
                        spec = format!("catalog:{catalog_name}");
                        break;
                    }
                }
            }
        }

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

pub fn build_project_manifest_root(
    dependencies: &BTreeMap<String, String>,
    development_dependencies: &BTreeMap<String, String>,
    optional_dependencies: &BTreeMap<String, String>,
    include_dev: bool,
) -> BTreeMap<String, String> {
    build_project_root_specs(
        dependencies,
        development_dependencies,
        optional_dependencies,
        include_dev,
    )
    .required
}

pub fn build_project_root_specs(
    dependencies: &BTreeMap<String, String>,
    development_dependencies: &BTreeMap<String, String>,
    optional_dependencies: &BTreeMap<String, String>,
    include_dev: bool,
) -> RootSpecSet {
    let mut required = dependencies.clone();

    if include_dev {
        for (name, range) in development_dependencies.iter() {
            required.entry(name.clone()).or_insert(range.clone());
        }
    }

    let optional = optional_dependencies.clone();

    for name in optional.keys() {
        required.remove(name);
    }

    RootSpecSet { required, optional }
}

pub fn apply_specs(
    specs: &BTreeMap<String, String>,
    workspace: Option<&Workspace>,
    catalog: Option<&CatalogConfig>,
    local_set: &mut BTreeSet<String>,
    mut protocol_map: Option<&mut BTreeMap<String, RegistryProtocol>>,
) -> Result<BTreeMap<String, String>> {
    let mut result = BTreeMap::new();

    for (name, value) in specs.iter() {
        if value.starts_with("workspace:") {
            local_set.insert(name.clone());

            if let Some(workspace_reference) = workspace {
                validate_workspace_spec(workspace_reference, name, value)?;
            }

            continue;
        }

        let resolved = if value.starts_with("catalog:") {
            resolve_catalog_spec(name, value, workspace, catalog)?
        } else {
            value.clone()
        };

        if let Some(map) = &mut protocol_map
            && let Some(protocol) = detect_manifest_protocol(&resolved)
        {
            map.insert(name.clone(), protocol);
        }

        result.insert(name.clone(), resolved);
    }

    Ok(result)
}

pub fn resolve_catalog_spec(
    name: &str,
    value: &str,
    workspace: Option<&Workspace>,
    catalog: Option<&CatalogConfig>,
) -> Result<String> {
    let selector = &value["catalog:".len()..];

    let (default_catalog, named_catalogs, root_path) = if let Some(workspace_reference) = workspace
    {
        (
            &workspace_reference.config.catalog,
            &workspace_reference.config.catalogs,
            workspace_reference.root.clone(),
        )
    } else if let Some(catalog_config) = catalog {
        (
            &catalog_config.catalog,
            &catalog_config.catalogs,
            PathBuf::from("."),
        )
    } else {
        return Err(SnpmError::WorkspaceConfig {
            path: PathBuf::from("."),
            reason: "catalog protocol used but no workspace or catalog configuration found".into(),
        });
    };

    let range_opt = if selector.is_empty() || selector == "default" {
        default_catalog.get(name)
    } else {
        named_catalogs
            .get(selector)
            .and_then(|entries| entries.get(name))
    };

    match range_opt {
        Some(range) => Ok(range.clone()),
        None => Err(SnpmError::WorkspaceConfig {
            path: root_path,
            reason: format!("no catalog entry found for dependency {name} and selector {value}"),
        }),
    }
}

pub fn detect_manifest_protocol(spec: &str) -> Option<RegistryProtocol> {
    if spec.starts_with("npm:") {
        Some(RegistryProtocol::npm())
    } else if is_git_spec(spec) {
        Some(RegistryProtocol::git())
    } else if spec.starts_with("jsr:") {
        Some(RegistryProtocol::jsr())
    } else if spec.starts_with("file:") {
        Some(RegistryProtocol::file())
    } else {
        None
    }
}

pub fn is_special_protocol_spec(spec: &str) -> bool {
    spec.starts_with("catalog:")
        || spec.starts_with("workspace:")
        || spec.starts_with("npm:")
        || is_git_spec(spec)
        || spec.starts_with("jsr:")
}

fn is_git_spec(spec: &str) -> bool {
    spec.starts_with("git:") || spec.starts_with("git+")
}

pub fn parse_requested_with_protocol(
    specs: &[String],
) -> (BTreeMap<String, String>, BTreeMap<String, RegistryProtocol>) {
    let mut ranges = BTreeMap::new();
    let mut protocols = BTreeMap::new();

    for spec in specs {
        let parsed = parse_requested_spec(spec);
        ranges.insert(parsed.name.clone(), parsed.range.clone());

        if let Some(protocol_str) = parsed.protocol.as_deref() {
            let protocol = match protocol_str {
                "npm" => RegistryProtocol::npm(),
                "git" => RegistryProtocol::git(),
                "jsr" => RegistryProtocol::jsr(),
                other if other.starts_with("git+") => RegistryProtocol::git(),
                other => RegistryProtocol::custom(other),
            };
            protocols.insert(parsed.name.clone(), protocol);
        }
    }

    (ranges, protocols)
}

pub fn parse_requested_spec(spec: &str) -> ParsedSpec {
    let mut protocol = None;
    let mut rest = spec;

    if let Some(index) = spec.find(':') {
        let (prefix, after) = spec.split_at(index);
        if !prefix.is_empty() {
            protocol = Some(prefix.to_string());
            rest = &after[1..];
        }
    }

    if let Some(without_at) = rest.strip_prefix('@') {
        if let Some(index) = without_at.rfind('@') {
            let (scope_and_name, range) = without_at.split_at(index);
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

    if let Some(index) = rest.rfind('@') {
        let (name, range) = rest.split_at(index);
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

pub fn parse_spec(spec: &str) -> (String, String) {
    if let Some(without_at) = spec.strip_prefix('@') {
        if let Some(index) = without_at.rfind('@') {
            let (scope_and_name, range) = without_at.split_at(index);
            let name = format!("@{}", scope_and_name);
            let requested = range.trim_start_matches('@').to_string();
            return (name, requested);
        } else {
            return (spec.to_string(), "latest".to_string());
        }
    }

    if let Some(index) = spec.rfind('@') {
        let (name, range) = spec.split_at(index);
        let requested = range.trim_start_matches('@').to_string();
        (name.to_string(), requested)
    } else {
        (spec.to_string(), "latest".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::build_project_root_specs;
    use std::collections::BTreeMap;

    #[test]
    fn optional_dependencies_override_required_roots() {
        let dependencies = BTreeMap::from([("left-pad".to_string(), "^1.0.0".to_string())]);
        let optional = BTreeMap::from([("left-pad".to_string(), "^2.0.0".to_string())]);

        let root_specs =
            build_project_root_specs(&dependencies, &BTreeMap::new(), &optional, false);

        assert!(root_specs.required.is_empty());
        assert_eq!(
            root_specs.optional.get("left-pad").map(String::as_str),
            Some("^2.0.0")
        );
    }

    #[test]
    fn build_project_root_specs_includes_dev_when_flagged() {
        let deps = BTreeMap::from([("react".to_string(), "^18.0.0".to_string())]);
        let dev = BTreeMap::from([("jest".to_string(), "^29.0.0".to_string())]);

        let specs = build_project_root_specs(&deps, &dev, &BTreeMap::new(), true);
        assert!(specs.required.contains_key("react"));
        assert!(specs.required.contains_key("jest"));
    }

    #[test]
    fn build_project_root_specs_excludes_dev_when_not_flagged() {
        let deps = BTreeMap::from([("react".to_string(), "^18.0.0".to_string())]);
        let dev = BTreeMap::from([("jest".to_string(), "^29.0.0".to_string())]);

        let specs = build_project_root_specs(&deps, &dev, &BTreeMap::new(), false);
        assert!(specs.required.contains_key("react"));
        assert!(!specs.required.contains_key("jest"));
    }

    #[test]
    fn build_project_root_specs_deps_take_priority_over_dev() {
        let deps = BTreeMap::from([("shared".to_string(), "^1.0.0".to_string())]);
        let dev = BTreeMap::from([("shared".to_string(), "^2.0.0".to_string())]);

        let specs = build_project_root_specs(&deps, &dev, &BTreeMap::new(), true);
        assert_eq!(
            specs.required.get("shared").map(String::as_str),
            Some("^1.0.0")
        );
    }

    #[test]
    fn parse_spec_simple() {
        let (name, range) = super::parse_spec("lodash@^4.0.0");
        assert_eq!(name, "lodash");
        assert_eq!(range, "^4.0.0");
    }

    #[test]
    fn parse_spec_scoped() {
        let (name, range) = super::parse_spec("@types/node@^18.0.0");
        assert_eq!(name, "@types/node");
        assert_eq!(range, "^18.0.0");
    }

    #[test]
    fn parse_spec_no_version() {
        let (name, range) = super::parse_spec("lodash");
        assert_eq!(name, "lodash");
        assert_eq!(range, "latest");
    }

    #[test]
    fn parse_spec_scoped_no_version() {
        let (name, range) = super::parse_spec("@types/node");
        assert_eq!(name, "@types/node");
        assert_eq!(range, "latest");
    }

    #[test]
    fn parse_requested_spec_with_protocol() {
        let parsed = super::parse_requested_spec("npm:@scope/pkg@^1.0.0");
        assert_eq!(parsed.protocol.as_deref(), Some("npm"));
        assert_eq!(parsed.name, "@scope/pkg");
        assert_eq!(parsed.range, "^1.0.0");
    }

    #[test]
    fn parse_requested_spec_no_protocol() {
        let parsed = super::parse_requested_spec("lodash@^4.0.0");
        assert_eq!(parsed.name, "lodash");
        assert_eq!(parsed.range, "^4.0.0");
    }

    #[test]
    fn detect_manifest_protocol_npm() {
        use crate::registry::RegistryProtocol;
        assert_eq!(
            super::detect_manifest_protocol("npm:foo@^1.0.0"),
            Some(RegistryProtocol::npm())
        );
    }

    #[test]
    fn detect_manifest_protocol_git() {
        use crate::registry::RegistryProtocol;
        assert_eq!(
            super::detect_manifest_protocol("git+https://github.com/foo/bar.git"),
            Some(RegistryProtocol::git())
        );
    }

    #[test]
    fn detect_manifest_protocol_jsr() {
        use crate::registry::RegistryProtocol;
        assert_eq!(
            super::detect_manifest_protocol("jsr:@std/path@^1.0.0"),
            Some(RegistryProtocol::jsr())
        );
    }

    #[test]
    fn detect_manifest_protocol_file() {
        use crate::registry::RegistryProtocol;
        assert_eq!(
            super::detect_manifest_protocol("file:../local-pkg"),
            Some(RegistryProtocol::file())
        );
    }

    #[test]
    fn detect_manifest_protocol_none() {
        assert_eq!(super::detect_manifest_protocol("^1.0.0"), None);
    }

    #[test]
    fn is_special_protocol_spec_examples() {
        assert!(super::is_special_protocol_spec("catalog:"));
        assert!(super::is_special_protocol_spec("catalog:build"));
        assert!(super::is_special_protocol_spec("workspace:*"));
        assert!(super::is_special_protocol_spec("npm:other@^1.0.0"));
        assert!(super::is_special_protocol_spec(
            "git+https://example.com/repo.git"
        ));
        assert!(super::is_special_protocol_spec("jsr:@std/path@^1.0.0"));
        assert!(!super::is_special_protocol_spec("^1.0.0"));
        assert!(!super::is_special_protocol_spec("~2.0.0"));
    }
}
