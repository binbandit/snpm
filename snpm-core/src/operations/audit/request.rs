use super::types::AuditOptions;
use crate::lockfile::Lockfile;
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, Serialize)]
pub(super) struct AuditNode {
    version: Option<String>,
    integrity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    requires: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dependencies: Option<BTreeMap<String, AuditNode>>,
    dev: bool,
}

#[derive(Debug, Serialize)]
pub(super) struct AuditRequest {
    name: String,
    version: String,
    requires: BTreeMap<String, String>,
    dependencies: BTreeMap<String, AuditNode>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    install: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    remove: Vec<String>,
    metadata: AuditMetadataRequest,
}

#[derive(Debug, Serialize)]
pub(super) struct AuditMetadataRequest {
    node_version: String,
    npm_version: String,
    platform: String,
}

pub(super) fn build_audit_request(
    lockfile: &Lockfile,
    project_name: &str,
    dev_names: &HashSet<String>,
    options: &AuditOptions,
) -> AuditRequest {
    let mut requires = BTreeMap::new();
    let mut dependencies = BTreeMap::new();

    for (name, root_dep) in &lockfile.root.dependencies {
        requires.insert(name.clone(), root_dep.requested.clone());
    }

    for pkg in lockfile.packages.values() {
        if !options.packages.is_empty() && !options.packages.contains(&pkg.name) {
            continue;
        }

        let is_dev = dev_names.contains(&pkg.name);
        if options.production_only && is_dev {
            continue;
        }
        if options.dev_only && !is_dev {
            continue;
        }

        let requires = build_package_requires(&pkg.dependencies);
        dependencies.insert(
            pkg.name.clone(),
            AuditNode {
                version: Some(pkg.version.clone()),
                integrity: pkg.integrity.clone(),
                requires,
                dependencies: None,
                dev: is_dev,
            },
        );
    }

    AuditRequest {
        name: project_name.to_string(),
        version: "0.0.0".to_string(),
        requires,
        dependencies,
        install: vec![],
        remove: vec![],
        metadata: AuditMetadataRequest {
            node_version: "20.0.0".to_string(),
            npm_version: "10.0.0".to_string(),
            platform: std::env::consts::OS.to_string(),
        },
    }
}

fn build_package_requires(
    dependencies: &BTreeMap<String, String>,
) -> Option<BTreeMap<String, String>> {
    let mut requires = BTreeMap::new();

    for (name, dep_key) in dependencies {
        if let Some((_, version)) = split_dep_key(dep_key) {
            requires.insert(name.clone(), version);
        }
    }

    if requires.is_empty() {
        None
    } else {
        Some(requires)
    }
}

fn split_dep_key(key: &str) -> Option<(String, String)> {
    let index = key.rfind('@')?;
    let (name, version_part) = key.split_at(index);
    Some((
        name.to_string(),
        version_part.trim_start_matches('@').to_string(),
    ))
}
