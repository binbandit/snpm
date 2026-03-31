mod packages;
mod root;

use super::types::Lockfile;
use crate::resolve::{ResolutionGraph, ResolutionRoot};

use packages::{build_package_nodes, populate_package_dependencies};
use root::build_root_dependencies;

pub fn to_graph(lockfile: &Lockfile) -> ResolutionGraph {
    let mut packages = build_package_nodes(lockfile);
    populate_package_dependencies(lockfile, &mut packages);

    ResolutionGraph {
        root: ResolutionRoot {
            dependencies: build_root_dependencies(lockfile),
        },
        packages,
    }
}

#[cfg(test)]
mod tests;
