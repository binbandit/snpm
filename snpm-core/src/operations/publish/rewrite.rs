//! Rewrite `workspace:` and `catalog:` dependency specs to concrete
//! registry ranges when packing/publishing.
//!
//! These protocols only mean something inside the monorepo. Shipping
//! them to the registry produces metadata an external `npm install`
//! can't resolve — a silent, broken publish. npm/pnpm/yarn/bun all
//! rewrite them at publish time; so do we, for both the tarball's
//! `package.json` and the publish payload.

use crate::operations::install::manifest::resolve_catalog_spec;
use crate::project::format_manifest_object;
use crate::workspace::CatalogConfig;
use crate::{Project, Result, SnpmError, Workspace};

use serde_json::Value;

use super::manifest::read_manifest_value;

const DEP_SECTIONS: [&str; 4] = [
    "dependencies",
    "devDependencies",
    "optionalDependencies",
    "peerDependencies",
];

/// Read `project`'s manifest and rewrite its `workspace:`/`catalog:`
/// dependency specs to concrete registry ranges, discovering the
/// enclosing workspace and catalog as needed. Used to build the publish
/// payload's version metadata.
pub(crate) fn prepare_manifest_for_publish(project: &Project) -> Result<Value> {
    let mut manifest = read_manifest_value(project)?;
    let workspace = Workspace::discover(&project.root)?;
    let catalog = CatalogConfig::load(&project.root)?;
    rewrite_published_manifest(&mut manifest, workspace.as_ref(), catalog.as_ref())?;
    Ok(manifest)
}

/// The bytes to write for `project`'s `package.json` inside a tarball,
/// or `None` when nothing needed rewriting (so the packer keeps the file
/// byte-for-byte). When a rewrite does happen the manifest is
/// re-serialized in npm's canonical field order.
pub(crate) fn rewritten_manifest_bytes(project: &Project) -> Result<Option<Vec<u8>>> {
    let mut manifest = read_manifest_value(project)?;
    let workspace = Workspace::discover(&project.root)?;
    let catalog = CatalogConfig::load(&project.root)?;

    if !rewrite_published_manifest(&mut manifest, workspace.as_ref(), catalog.as_ref())? {
        return Ok(None);
    }

    let Value::Object(object) = manifest else {
        return Ok(None);
    };

    let formatted = format_manifest_object(object, &project.manifest_path)?;
    Ok(Some(formatted.into_bytes()))
}

/// Rewrite every `workspace:`/`catalog:` spec in the manifest's
/// dependency sections in place. Errors if a `workspace:` dep names a
/// package not present in the workspace (that would publish an
/// unresolvable range). Returns whether any spec was rewritten.
pub(crate) fn rewrite_published_manifest(
    manifest: &mut Value,
    workspace: Option<&Workspace>,
    catalog: Option<&CatalogConfig>,
) -> Result<bool> {
    let mut changed = false;
    for section in DEP_SECTIONS {
        let Some(Value::Object(map)) = manifest.get_mut(section) else {
            continue;
        };
        for (name, spec_value) in map.iter_mut() {
            let Value::String(spec) = spec_value else {
                continue;
            };
            if let Some(rewritten) = rewrite_spec(name, spec, workspace, catalog)? {
                *spec_value = Value::String(rewritten);
                changed = true;
            }
        }
    }
    Ok(changed)
}

fn rewrite_spec(
    name: &str,
    spec: &str,
    workspace: Option<&Workspace>,
    catalog: Option<&CatalogConfig>,
) -> Result<Option<String>> {
    if let Some(rest) = spec.strip_prefix("workspace:") {
        // pnpm's aliased form `workspace:foo@^` / `workspace:@scope/foo@~`:
        // the edge name differs from the sibling package, so the published
        // spec must become an npm alias (`npm:foo@^1.2.3`).
        if let Some((target, range)) = split_workspace_alias(rest) {
            let rewritten = concrete_range(range, || sibling_version(target, workspace))?;
            return Ok(Some(format!("npm:{target}@{rewritten}")));
        }

        return Ok(Some(concrete_range(rest, || {
            sibling_version(name, workspace)
        })?));
    }

    if spec.starts_with("catalog:") {
        return Ok(Some(resolve_catalog_spec(name, spec, workspace, catalog)?));
    }

    Ok(None)
}

/// Turn a `workspace:` range shorthand into a real registry range,
/// resolving the sibling's version lazily (only the shorthand forms need
/// it; an explicit range like `^1.2.3` passes through untouched).
fn concrete_range(range: &str, version: impl FnOnce() -> Result<String>) -> Result<String> {
    Ok(match range {
        "" | "*" => version()?,
        "~" => format!("~{}", version()?),
        "^" => format!("^{}", version()?),
        other => other.to_string(),
    })
}

/// Split pnpm's aliased workspace form `<target>@<range>` (e.g.
/// `foo@^`, `@scope/foo@~`). Returns `None` for plain ranges, which
/// contain no `@` past the scope position.
fn split_workspace_alias(rest: &str) -> Option<(&str, &str)> {
    let at = rest.rfind('@')?;
    if at == 0 {
        return None;
    }
    let (target, range) = (&rest[..at], &rest[at + 1..]);
    if target.is_empty() {
        return None;
    }
    Some((target, range))
}

fn sibling_version(name: &str, workspace: Option<&Workspace>) -> Result<String> {
    let workspace = workspace.ok_or_else(|| SnpmError::ManifestInvalid {
        path: std::path::PathBuf::from("package.json"),
        reason: format!(
            "dependency {name} uses the workspace: protocol but the project is not in a workspace"
        ),
    })?;

    let project = workspace
        .project_by_name(name)
        .ok_or_else(|| SnpmError::WorkspaceConfig {
            path: workspace.root.clone(),
            reason: format!("workspace dependency {name} was not found among workspace packages"),
        })?;

    project
        .manifest
        .version
        .clone()
        .ok_or_else(|| SnpmError::ManifestInvalid {
            path: project.manifest_path.clone(),
            reason: format!("workspace package {name} has no version to publish against"),
        })
}

#[cfg(test)]
mod tests {
    use super::rewrite_published_manifest;
    use crate::Project;
    use crate::project::Manifest;
    use crate::workspace::types::{Workspace, WorkspaceConfig};
    use serde_json::json;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn workspace_with(name: &str, version: &str) -> Workspace {
        let project = Project {
            manifest_path: PathBuf::from("/ws/pkg/package.json"),
            root: PathBuf::from("/ws/pkg"),
            manifest: Manifest {
                name: Some(name.to_string()),
                version: Some(version.to_string()),
                ..Manifest::default()
            },
        };
        Workspace {
            root: PathBuf::from("/ws"),
            projects: vec![project],
            config: WorkspaceConfig {
                packages: Vec::new(),
                catalog: BTreeMap::new(),
                catalogs: BTreeMap::new(),
                only_built_dependencies: Vec::new(),
                ignored_built_dependencies: Vec::new(),
                disable_global_virtual_store_for_packages: None,
                hoisting: None,
            },
        }
    }

    #[test]
    fn rewrites_workspace_protocol_forms() {
        let ws = workspace_with("@acme/utils", "1.4.2");
        let mut manifest = json!({
            "dependencies": {
                "@acme/utils": "workspace:^",
                "left-pad": "^1.0.0"
            },
            "devDependencies": { "@acme/utils": "workspace:*" },
            "peerDependencies": { "@acme/utils": "workspace:~" }
        });

        rewrite_published_manifest(&mut manifest, Some(&ws), None).unwrap();

        assert_eq!(manifest["dependencies"]["@acme/utils"], json!("^1.4.2"));
        assert_eq!(manifest["dependencies"]["left-pad"], json!("^1.0.0"));
        assert_eq!(manifest["devDependencies"]["@acme/utils"], json!("1.4.2"));
        assert_eq!(manifest["peerDependencies"]["@acme/utils"], json!("~1.4.2"));
    }

    #[test]
    fn rewrites_explicit_workspace_range() {
        let ws = workspace_with("@acme/utils", "1.4.2");
        let mut manifest = json!({ "dependencies": { "@acme/utils": "workspace:^2.0.0" } });
        rewrite_published_manifest(&mut manifest, Some(&ws), None).unwrap();
        assert_eq!(manifest["dependencies"]["@acme/utils"], json!("^2.0.0"));
    }

    #[test]
    fn rewrites_aliased_workspace_spec_to_npm_alias() {
        let ws = workspace_with("@acme/utils", "1.4.2");
        let mut manifest = json!({
            "dependencies": {
                "utils": "workspace:@acme/utils@^",
                "utils-exact": "workspace:@acme/utils@*"
            }
        });

        rewrite_published_manifest(&mut manifest, Some(&ws), None).unwrap();

        assert_eq!(
            manifest["dependencies"]["utils"],
            json!("npm:@acme/utils@^1.4.2")
        );
        assert_eq!(
            manifest["dependencies"]["utils-exact"],
            json!("npm:@acme/utils@1.4.2")
        );
    }

    #[test]
    fn errors_when_workspace_dep_missing() {
        let ws = workspace_with("@acme/other", "1.0.0");
        let mut manifest = json!({ "dependencies": { "@acme/utils": "workspace:*" } });
        assert!(rewrite_published_manifest(&mut manifest, Some(&ws), None).is_err());
    }
}
