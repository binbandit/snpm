mod env;
mod file;
mod types;
mod url;

pub use env::{
    expand_env_vars, read_allow_scripts_from_env, read_min_package_age_from_env,
    read_min_package_cache_age_from_env,
};
pub use file::{apply_rc_file, read_registry_config};
pub use types::RegistryConfig;
pub(crate) use url::host_from_url;
pub use url::normalize_registry_url;
