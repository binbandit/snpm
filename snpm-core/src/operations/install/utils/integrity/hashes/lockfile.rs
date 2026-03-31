use crate::resolve::ResolutionGraph;

use sha2::{Digest, Sha256};

pub const NO_PATCH_HASH: &str = "none";

pub fn compute_lockfile_hash(graph: &ResolutionGraph) -> String {
    let mut hasher = Sha256::new();

    for (name, dep) in &graph.root.dependencies {
        hasher.update(name.as_bytes());
        hasher.update(dep.requested.as_bytes());
        hasher.update(dep.resolved.name.as_bytes());
        hasher.update(dep.resolved.version.as_bytes());
    }

    for (id, package) in &graph.packages {
        hasher.update(id.name.as_bytes());
        hasher.update(id.version.as_bytes());
        hasher.update(package.id.name.as_bytes());
        hasher.update(package.id.version.as_bytes());
    }

    format!("{:x}", hasher.finalize())
}
