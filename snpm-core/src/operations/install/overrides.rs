//! Manifest-declared dependency overrides, shared by every loader.
//!
//! Overrides come from four manifest sources — yarn `resolutions`, npm
//! top-level `overrides`, `pnpm.overrides`, and `snpm.overrides` — plus
//! the standalone `snpm-overrides.yaml`. The single-project planner,
//! the workspace planner, and outdated/upgrade must all honor the same
//! set, or a pin silently applies in one context and not another.

use crate::{Project, Result};

use std::collections::BTreeMap;

/// Merge every override form `project`'s manifest declares into
/// `overrides`, later sources winning: resolutions, npm `overrides`,
/// `pnpm.overrides`, `snpm.overrides`.
///
/// npm `overrides` supports the flat string form (`"foo": "1.2.3"`) and
/// the `"."` self-key of the nested form (`"foo": { ".": "1.2.3", ... }`),
/// which pins `foo` itself. A `$name` value copies the spec of a direct
/// dependency, as npm allows. The per-parent nested sub-keys (scoped
/// overrides) can't map onto snpm's flat model and are skipped rather
/// than silently mis-applied.
pub(crate) fn merge_manifest_overrides(
    project: &Project,
    overrides: &mut BTreeMap<String, String>,
) -> Result<()> {
    for (name, range) in &project.manifest.resolutions {
        overrides.insert(name.clone(), range.clone());
    }

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

    Ok(())
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
