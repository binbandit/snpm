mod policy;
mod runner;

pub(crate) use policy::is_dep_script_allowed;
pub use runner::{run_install_scripts, run_install_scripts_for_projects, run_project_scripts};
