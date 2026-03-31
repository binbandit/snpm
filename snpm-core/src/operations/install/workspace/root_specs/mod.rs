mod collect;
mod validate;

pub use collect::{
    collect_workspace_root_deps, collect_workspace_root_specs, insert_workspace_root_dep,
};
pub use validate::validate_workspace_spec;

#[cfg(test)]
mod tests;
