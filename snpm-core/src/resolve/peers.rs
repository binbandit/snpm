use crate::resolve::types::ResolutionGraph;
use crate::{Result, SnpmError};
use std::collections::BTreeMap;

pub fn validate_peers(graph: &ResolutionGraph) -> Result<()> {
    let mut versions_by_name = BTreeMap::new();

    for package in graph.packages.values() {
        if let Ok(ver) = snpm_semver::parse_version(&package.id.version) {
            versions_by_name
                .entry(package.id.name.clone())
                .or_insert_with(Vec::new)
                .push(ver);
        }
    }

    for package in graph.packages.values() {
        if package.peer_dependencies.is_empty() {
            continue;
        }

        for (peer_name, peer_range) in package.peer_dependencies.iter() {
            let range_set = crate::version::parse_range_set(peer_name, peer_range)?;

            let candidates = match versions_by_name.get(peer_name) {
                Some(list) => list,
                None => {
                    return Err(SnpmError::ResolutionFailed {
                        name: package.id.name.clone(),
                        range: peer_range.clone(),
                        reason: format!("missing peer dependency {peer_name}"),
                    });
                }
            };

            let mut satisfied = false;

            for ver in candidates {
                if range_set.matches(ver) {
                    satisfied = true;
                    break;
                }
            }

            if !satisfied {
                let installed = candidates
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");

                return Err(SnpmError::ResolutionFailed {
                    name: package.id.name.clone(),
                    range: peer_range.clone(),
                    reason: format!(
                        "peer dependency {peer_name}@{peer_range} is not satisfied; installed versions: {installed}"
                    ),
                });
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::types::*;
    use std::collections::BTreeMap;

    fn make_package(name: &str, version: &str) -> (PackageId, ResolvedPackage) {
        let id = PackageId {
            name: name.to_string(),
            version: version.to_string(),
        };
        let pkg = ResolvedPackage {
            id: id.clone(),
            tarball: String::new(),
            integrity: None,
            dependencies: BTreeMap::new(),
            peer_dependencies: BTreeMap::new(),
            bundled_dependencies: None,
            has_bin: false,
        };
        (id, pkg)
    }

    #[test]
    fn validates_satisfied_peer_dependency() {
        let (react_id, react) = make_package("react", "18.2.0");
        let (plugin_id, mut plugin) = make_package("react-plugin", "1.0.0");
        plugin
            .peer_dependencies
            .insert("react".to_string(), "^18.0.0".to_string());

        let mut packages = BTreeMap::new();
        packages.insert(react_id.clone(), react);
        packages.insert(plugin_id, plugin);

        let graph = ResolutionGraph {
            root: ResolutionRoot {
                dependencies: BTreeMap::new(),
            },
            packages,
        };

        assert!(validate_peers(&graph).is_ok());
    }

    #[test]
    fn rejects_unsatisfied_peer_dependency() {
        let (react_id, react) = make_package("react", "17.0.2");
        let (plugin_id, mut plugin) = make_package("react-plugin", "1.0.0");
        plugin
            .peer_dependencies
            .insert("react".to_string(), "^18.0.0".to_string());

        let mut packages = BTreeMap::new();
        packages.insert(react_id, react);
        packages.insert(plugin_id, plugin);

        let graph = ResolutionGraph {
            root: ResolutionRoot {
                dependencies: BTreeMap::new(),
            },
            packages,
        };

        assert!(validate_peers(&graph).is_err());
    }

    #[test]
    fn rejects_missing_peer_dependency() {
        let (plugin_id, mut plugin) = make_package("react-plugin", "1.0.0");
        plugin
            .peer_dependencies
            .insert("react".to_string(), "^18.0.0".to_string());

        let mut packages = BTreeMap::new();
        packages.insert(plugin_id, plugin);

        let graph = ResolutionGraph {
            root: ResolutionRoot {
                dependencies: BTreeMap::new(),
            },
            packages,
        };

        assert!(validate_peers(&graph).is_err());
    }

    #[test]
    fn no_peer_deps_passes() {
        let (id, pkg) = make_package("lodash", "4.17.21");

        let mut packages = BTreeMap::new();
        packages.insert(id, pkg);

        let graph = ResolutionGraph {
            root: ResolutionRoot {
                dependencies: BTreeMap::new(),
            },
            packages,
        };

        assert!(validate_peers(&graph).is_ok());
    }

    #[test]
    fn empty_graph_passes() {
        let graph = ResolutionGraph {
            root: ResolutionRoot {
                dependencies: BTreeMap::new(),
            },
            packages: BTreeMap::new(),
        };

        assert!(validate_peers(&graph).is_ok());
    }
}
