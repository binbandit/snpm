mod dependencies;
mod paths;
mod populate;

pub(in crate::operations::install::workspace) use dependencies::link_store_dependencies;
pub(in crate::operations::install::workspace) use paths::rebuild_virtual_store_paths;
pub(in crate::operations::install::workspace) use populate::populate_virtual_store;

#[cfg(test)]
mod tests;
