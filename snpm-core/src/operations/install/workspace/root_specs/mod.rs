mod collect;
mod validate;

pub(crate) use collect::collect_workspace_root_specs_with_overrides;
pub use collect::{
    collect_workspace_root_deps, collect_workspace_root_specs, insert_workspace_root_dep,
};
pub(crate) use validate::is_local_workspace_dependency;
pub use validate::validate_workspace_spec;

#[cfg(test)]
mod tests;
