use crate::resolve::types::ResolutionGraph;
use crate::{Result, SnpmError};
use std::collections::BTreeMap;

pub fn validate_peers(graph: &ResolutionGraph) -> Result<()> {
    let mut versions_by_name = BTreeMap::new();

    for package in graph.packages.values() {
        if let Ok(ver) = snpm_semver::Version::parse(&package.id.version) {
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
