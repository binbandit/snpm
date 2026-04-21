use crate::Result;
use crate::resolve::{PackageId, ResolutionGraph};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub(in crate::operations::install::workspace) fn link_store_dependencies(
    virtual_store_paths: &BTreeMap<PackageId, PathBuf>,
    graph: &ResolutionGraph,
) -> Result<()> {
    crate::linker::link_virtual_dependencies(virtual_store_paths, graph)
}
