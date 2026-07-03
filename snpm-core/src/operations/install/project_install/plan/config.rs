use crate::workspace::{CatalogConfig, OverridesConfig};
use crate::{Project, Result, Workspace};

use std::collections::BTreeMap;

pub(super) fn load_catalog(
    project: &Project,
    workspace: Option<&Workspace>,
) -> Result<Option<CatalogConfig>> {
    if workspace.is_some() {
        return Ok(None);
    }

    CatalogConfig::load(&project.root)
}

pub(super) fn load_overrides(
    project: &Project,
    workspace: Option<&Workspace>,
) -> Result<BTreeMap<String, String>> {
    let root = workspace
        .map(|workspace| &workspace.root)
        .unwrap_or(&project.root);
    let mut overrides = OverridesConfig::load(root)?
        .map(|config| config.overrides)
        .unwrap_or_default();

    crate::operations::install::overrides::merge_manifest_overrides(project, &mut overrides)?;

    Ok(overrides)
}

#[cfg(test)]
mod tests {
    use super::load_overrides;
    use crate::Project;
    use crate::project::Manifest;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn project_with(overrides: serde_json::Value, deps: &[(&str, &str)]) -> Project {
        let dependencies = deps
            .iter()
            .map(|(name, spec)| (name.to_string(), spec.to_string()))
            .collect::<BTreeMap<_, _>>();
        let overrides = overrides
            .as_object()
            .expect("overrides must be an object")
            .iter()
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect::<BTreeMap<_, _>>();

        Project {
            manifest_path: PathBuf::from("/app/package.json"),
            root: PathBuf::from("/app"),
            manifest: Manifest {
                name: Some("app".to_string()),
                version: Some("1.0.0".to_string()),
                dependencies,
                overrides,
                ..Manifest::default()
            },
        }
    }

    #[test]
    fn nested_override_dot_self_key_pins_the_package() {
        let project = project_with(
            serde_json::json!({ "foo": { ".": "1.0.1", "bar": "2.0.0" } }),
            &[],
        );
        let overrides = load_overrides(&project, None).unwrap();
        assert_eq!(overrides.get("foo").map(String::as_str), Some("1.0.1"));
        // The scoped sub-key (`bar` under `foo`) has no flat mapping.
        assert!(!overrides.contains_key("bar"));
    }

    #[test]
    fn dollar_reference_resolves_to_direct_dependency_spec() {
        let project = project_with(serde_json::json!({ "foo": "$bar" }), &[("bar", "^2.3.4")]);
        let overrides = load_overrides(&project, None).unwrap();
        assert_eq!(overrides.get("foo").map(String::as_str), Some("^2.3.4"));
    }

    #[test]
    fn dollar_reference_without_matching_dependency_is_an_error() {
        let project = project_with(serde_json::json!({ "foo": "$missing" }), &[]);
        let error = load_overrides(&project, None).unwrap_err();
        assert!(error.to_string().contains("$missing"));
    }

    #[test]
    fn flat_string_override_still_applies() {
        let project = project_with(serde_json::json!({ "foo": "1.2.3" }), &[]);
        let overrides = load_overrides(&project, None).unwrap();
        assert_eq!(overrides.get("foo").map(String::as_str), Some("1.2.3"));
    }
}
