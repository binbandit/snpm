use super::types::AuditOptions;
use crate::lockfile::Lockfile;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashSet};

const MAX_PATHS_PER_FINDING: usize = 100;

#[derive(Debug, Serialize)]
#[serde(transparent)]
pub(super) struct AuditRequest {
    pub packages: BTreeMap<String, Vec<String>>,
}

#[derive(Debug)]
pub(super) struct AuditRequestData {
    pub request: AuditRequest,
    pub total_packages: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PathInfo {
    pub paths: Vec<String>,
    pub dev: bool,
    pub optional: bool,
}

pub(super) type AuditPathIndex = BTreeMap<String, BTreeMap<String, PathInfo>>;

pub(super) fn build_audit_request(
    lockfile: &Lockfile,
    dev_root_names: &HashSet<String>,
    options: &AuditOptions,
) -> AuditRequestData {
    let package_filters: HashSet<&str> = options.packages.iter().map(String::as_str).collect();
    let mut versions_by_name: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    walk_audited_packages(
        lockfile,
        dev_root_names,
        options,
        &mut |package_name, version, _, _, _| {
            if !package_filters.is_empty() && !package_filters.contains(package_name) {
                return;
            }

            versions_by_name
                .entry(package_name.to_string())
                .or_default()
                .insert(version.to_string());
        },
    );

    let total_packages = versions_by_name.values().map(BTreeSet::len).sum();
    let packages = versions_by_name
        .into_iter()
        .map(|(name, versions)| (name, versions.into_iter().collect()))
        .collect();

    AuditRequestData {
        request: AuditRequest { packages },
        total_packages,
    }
}

pub(super) fn build_audit_path_index(
    lockfile: &Lockfile,
    dev_root_names: &HashSet<String>,
    options: &AuditOptions,
    vulnerable_names: &HashSet<String>,
) -> AuditPathIndex {
    let mut paths = AuditPathIndex::new();

    if vulnerable_names.is_empty() {
        return paths;
    }

    walk_audited_packages(
        lockfile,
        dev_root_names,
        options,
        &mut |package_name, version, path, is_dev, is_optional| {
            if !vulnerable_names.contains(package_name) {
                return;
            }

            let by_version = paths.entry(package_name.to_string()).or_default();
            let info = by_version
                .entry(version.to_string())
                .or_insert_with(|| PathInfo {
                    paths: Vec::new(),
                    dev: is_dev,
                    optional: is_optional,
                });

            if !is_dev {
                info.dev = false;
            }
            if !is_optional {
                info.optional = false;
            }
            if info.paths.len() >= MAX_PATHS_PER_FINDING
                || info.paths.iter().any(|existing| existing == path)
            {
                return;
            }

            info.paths.push(path.to_string());
        },
    );

    paths
}

fn walk_audited_packages(
    lockfile: &Lockfile,
    dev_root_names: &HashSet<String>,
    options: &AuditOptions,
    visit: &mut impl FnMut(&str, &str, &str, bool, bool),
) {
    for (alias, root_dep) in &lockfile.root.dependencies {
        let is_dev_root = dev_root_names.contains(alias.as_str());
        if options.production_only && is_dev_root {
            continue;
        }
        if options.dev_only && !is_dev_root {
            continue;
        }

        let Some(version) = &root_dep.version else {
            continue;
        };

        let resolved_name = root_dep.package.as_deref().unwrap_or(alias.as_str());
        let root_key = format!("{resolved_name}@{version}");
        let mut trail = Vec::new();
        let mut in_trail = HashSet::new();

        walk_package(
            lockfile,
            &root_key,
            alias,
            is_dev_root,
            root_dep.optional,
            &mut trail,
            &mut in_trail,
            visit,
        );
    }
}

fn walk_package(
    lockfile: &Lockfile,
    package_key: &str,
    label: &str,
    is_dev: bool,
    is_optional: bool,
    trail: &mut Vec<String>,
    in_trail: &mut HashSet<String>,
    visit: &mut impl FnMut(&str, &str, &str, bool, bool),
) {
    let Some(package) = lockfile.packages.get(package_key) else {
        return;
    };

    if !in_trail.insert(package_key.to_string()) {
        return;
    }

    trail.push(label.to_string());
    let joined_path = trail.join(">");
    visit(
        package.name.as_str(),
        package.version.as_str(),
        joined_path.as_str(),
        is_dev,
        is_optional,
    );

    for (child_label, child_key) in &package.dependencies {
        walk_package(
            lockfile,
            child_key,
            child_label,
            is_dev,
            is_optional,
            trail,
            in_trail,
            visit,
        );
    }

    trail.pop();
    in_trail.remove(package_key);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lockfile::{LockPackage, LockRoot, LockRootDependency, Lockfile};
    use std::collections::{BTreeMap, HashSet};

    fn sample_lockfile() -> Lockfile {
        Lockfile {
            version: 1,
            root: LockRoot {
                dependencies: BTreeMap::from([
                    (
                        "prod-root".to_string(),
                        LockRootDependency {
                            requested: "^1.0.0".to_string(),
                            package: None,
                            version: Some("1.0.0".to_string()),
                            optional: false,
                        },
                    ),
                    (
                        "dev-root".to_string(),
                        LockRootDependency {
                            requested: "^1.0.0".to_string(),
                            package: None,
                            version: Some("1.0.0".to_string()),
                            optional: false,
                        },
                    ),
                    (
                        "optional-root".to_string(),
                        LockRootDependency {
                            requested: "^1.0.0".to_string(),
                            package: None,
                            version: Some("1.0.0".to_string()),
                            optional: true,
                        },
                    ),
                ]),
            },
            packages: BTreeMap::from([
                (
                    "prod-root@1.0.0".to_string(),
                    LockPackage {
                        name: "prod-root".to_string(),
                        version: "1.0.0".to_string(),
                        tarball: String::new(),
                        integrity: None,
                        dependencies: BTreeMap::from([(
                            "shared".to_string(),
                            "shared@1.0.0".to_string(),
                        )]),
                        bundled_dependencies: None,
                        has_bin: false,
                        bin: None,
                    },
                ),
                (
                    "dev-root@1.0.0".to_string(),
                    LockPackage {
                        name: "dev-root".to_string(),
                        version: "1.0.0".to_string(),
                        tarball: String::new(),
                        integrity: None,
                        dependencies: BTreeMap::from([(
                            "shared".to_string(),
                            "shared@1.0.0".to_string(),
                        )]),
                        bundled_dependencies: None,
                        has_bin: false,
                        bin: None,
                    },
                ),
                (
                    "optional-root@1.0.0".to_string(),
                    LockPackage {
                        name: "optional-root".to_string(),
                        version: "1.0.0".to_string(),
                        tarball: String::new(),
                        integrity: None,
                        dependencies: BTreeMap::from([(
                            "shared".to_string(),
                            "shared@1.0.0".to_string(),
                        )]),
                        bundled_dependencies: None,
                        has_bin: false,
                        bin: None,
                    },
                ),
                (
                    "shared@1.0.0".to_string(),
                    LockPackage {
                        name: "shared".to_string(),
                        version: "1.0.0".to_string(),
                        tarball: String::new(),
                        integrity: None,
                        dependencies: BTreeMap::new(),
                        bundled_dependencies: None,
                        has_bin: false,
                        bin: None,
                    },
                ),
            ]),
        }
    }

    #[test]
    fn build_audit_request_respects_prod_filter() {
        let lockfile = sample_lockfile();
        let dev_roots = HashSet::from(["dev-root".to_string()]);
        let options = AuditOptions {
            production_only: true,
            ..AuditOptions::default()
        };

        let request = build_audit_request(&lockfile, &dev_roots, &options);

        assert_eq!(request.total_packages, 3);
        assert_eq!(
            request.request.packages,
            BTreeMap::from([
                ("optional-root".to_string(), vec!["1.0.0".to_string()]),
                ("prod-root".to_string(), vec!["1.0.0".to_string()]),
                ("shared".to_string(), vec!["1.0.0".to_string()]),
            ]),
        );
    }

    #[test]
    fn build_audit_path_index_records_each_root_path() {
        let lockfile = sample_lockfile();
        let dev_roots = HashSet::from(["dev-root".to_string()]);
        let vulnerable_names = HashSet::from(["shared".to_string()]);
        let options = AuditOptions::default();

        let index = build_audit_path_index(&lockfile, &dev_roots, &options, &vulnerable_names);
        let shared = &index["shared"]["1.0.0"];

        assert_eq!(
            shared.paths,
            vec![
                "dev-root>shared".to_string(),
                "optional-root>shared".to_string(),
                "prod-root>shared".to_string(),
            ],
        );
        assert!(!shared.dev);
        assert!(!shared.optional);
    }
}
