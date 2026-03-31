mod local;
mod projects;
mod virtual_store;

pub use local::link_local_workspace_deps;
pub(super) use projects::link_project_dependencies;
pub(super) use virtual_store::{
    link_store_dependencies, populate_virtual_store, rebuild_virtual_store_paths,
};
