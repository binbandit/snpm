pub mod install;
pub mod run;

pub use install::{InstallOptions, install, remove};
pub use run::run_script;
