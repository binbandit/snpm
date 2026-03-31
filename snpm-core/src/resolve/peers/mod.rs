mod index;
mod validate;

use crate::Result;
use crate::resolve::types::ResolutionGraph;

use index::collect_versions_by_name;
use validate::validate_package_peers;

pub fn validate_peers(graph: &ResolutionGraph) -> Result<()> {
    let versions_by_name = collect_versions_by_name(graph);

    for package in graph.packages.values() {
        validate_package_peers(package, &versions_by_name)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests;
