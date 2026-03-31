use crate::resolve::types::ResolvedPackage;
use crate::{Result, SnpmError};

use std::collections::BTreeMap;

pub(super) fn validate_package_peers(
    package: &ResolvedPackage,
    versions_by_name: &BTreeMap<String, Vec<snpm_semver::Version>>,
) -> Result<()> {
    if package.peer_dependencies.is_empty() {
        return Ok(());
    }

    for (peer_name, peer_range) in &package.peer_dependencies {
        let range_set = crate::version::parse_range_set(peer_name, peer_range)?;

        let candidates = versions_by_name
            .get(peer_name)
            .ok_or_else(|| missing_peer(package, peer_name, peer_range))?;

        if candidates.iter().any(|version| range_set.matches(version)) {
            continue;
        }

        return Err(unsatisfied_peer(package, peer_name, peer_range, candidates));
    }

    Ok(())
}

fn missing_peer(package: &ResolvedPackage, peer_name: &str, peer_range: &str) -> SnpmError {
    SnpmError::ResolutionFailed {
        name: package.id.name.clone(),
        range: peer_range.to_string(),
        reason: format!("missing peer dependency {peer_name}"),
    }
}

fn unsatisfied_peer(
    package: &ResolvedPackage,
    peer_name: &str,
    peer_range: &str,
    candidates: &[snpm_semver::Version],
) -> SnpmError {
    let installed = candidates
        .iter()
        .map(|version| version.to_string())
        .collect::<Vec<_>>()
        .join(", ");

    SnpmError::ResolutionFailed {
        name: package.id.name.clone(),
        range: peer_range.to_string(),
        reason: format!(
            "peer dependency {peer_name}@{peer_range} is not satisfied; installed versions: {installed}"
        ),
    }
}
