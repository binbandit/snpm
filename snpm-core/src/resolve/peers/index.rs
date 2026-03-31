use crate::resolve::types::ResolutionGraph;

use std::collections::BTreeMap;

pub(super) fn collect_versions_by_name(
    graph: &ResolutionGraph,
) -> BTreeMap<String, Vec<snpm_semver::Version>> {
    let mut versions_by_name = BTreeMap::new();

    for package in graph.packages.values() {
        if let Ok(version) = snpm_semver::parse_version(&package.id.version) {
            versions_by_name
                .entry(package.id.name.clone())
                .or_insert_with(Vec::new)
                .push(version);
        }
    }

    versions_by_name
}
