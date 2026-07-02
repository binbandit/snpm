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

    for (name, range) in &project.manifest.resolutions {
        overrides.insert(name.clone(), range.clone());
    }

    // npm/Bun top-level `overrides`. Supports the flat string form
    // (`"foo": "1.2.3"`) and the `"."` self-key of npm's nested form
    // (`"foo": { ".": "1.2.3", ... }`), which pins `foo` itself. A `$name`
    // value copies the spec of a direct dependency, as npm allows. The
    // per-parent nested sub-keys (scoped overrides) can't map onto snpm's
    // flat model and are skipped rather than silently mis-applied.
    for (name, value) in &project.manifest.overrides {
        let Some(raw) = flat_override_range(value) else {
            continue;
        };
        match resolve_override_reference(&raw, project) {
            Some(resolved) => {
                overrides.insert(name.clone(), resolved);
            }
            // npm hard-errors on an unresolvable $reference. Silently
            // dropping it could quietly disable a security pin.
            None => {
                return Err(crate::SnpmError::ManifestInvalid {
                    path: project.manifest_path.clone(),
                    reason: format!(
                        "override for {name} references {raw}, which does not match any direct dependency"
                    ),
                });
            }
        }
    }

    if let Some(pnpm) = &project.manifest.pnpm {
        for (name, range) in &pnpm.overrides {
            overrides.insert(name.clone(), range.clone());
        }
    }

    if let Some(snpm) = &project.manifest.snpm {
        for (name, range) in &snpm.overrides {
            overrides.insert(name.clone(), range.clone());
        }
    }

    Ok(overrides)
}

/// Extract the flat override range for an npm `overrides` entry: a bare
/// string, or the `"."` self-key of the nested object form. Nested-only
/// objects (scoped per-parent overrides) have no flat mapping and yield
/// `None`.
fn flat_override_range(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(range) => Some(range.clone()),
        serde_json::Value::Object(map) => map
            .get(".")
            .and_then(|dot| dot.as_str())
            .map(str::to_string),
        _ => None,
    }
}

/// Resolve an npm `$name` override reference to the referenced direct
/// dependency's spec. A non-reference range passes through unchanged;
/// a reference with no matching direct dependency yields `None`.
fn resolve_override_reference(range: &str, project: &Project) -> Option<String> {
    match range.strip_prefix('$') {
        Some(referenced) => direct_dependency_spec(project, referenced),
        None => Some(range.to_string()),
    }
}

fn direct_dependency_spec(project: &Project, name: &str) -> Option<String> {
    let manifest = &project.manifest;
    manifest
        .dependencies
        .get(name)
        .or_else(|| manifest.dev_dependencies.get(name))
        .or_else(|| manifest.optional_dependencies.get(name))
        .cloned()
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
