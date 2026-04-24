mod env;
mod file;
mod types;
mod url;

pub use env::{
    expand_env_vars, parse_package_name_list, read_allow_scripts_from_env,
    read_disable_global_virtual_store_for_packages_from_env, read_min_package_age_from_env,
    read_min_package_cache_age_from_env,
};
pub use file::{apply_rc_file, read_registry_config};
pub use types::RegistryConfig;
pub(crate) use url::host_from_url;
pub use url::normalize_registry_url;
