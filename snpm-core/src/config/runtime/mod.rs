mod dirs;
mod env;

use super::rc::{
    read_allow_scripts_from_env, read_min_package_age_from_env,
    read_min_package_cache_age_from_env, read_registry_config,
};
use super::{AuthScheme, HoistingMode, LinkBackend, SnpmConfig};

use dirs::resolve_home_dirs;
use env::{apply_auth_env, apply_default_registry_env, apply_install_env, read_logging_env};

impl SnpmConfig {
    pub fn from_env() -> Self {
        let (cache_dir, data_dir) = resolve_home_dirs();
        let allow_scripts = read_allow_scripts_from_env();
        let min_package_age_days = read_min_package_age_from_env();
        let min_package_cache_age_days = read_min_package_cache_age_from_env();

        let runtime_config = read_registry_config();
        let runtime_config_default_auth_basic = runtime_config.default_auth_basic;

        let mut default_registry = runtime_config.default_registry;
        let scoped_registries = runtime_config.scoped;
        let registry_auth = runtime_config.registry_auth;
        let registry_auth_schemes = runtime_config.registry_auth_schemes;
        let mut default_registry_auth_token = runtime_config.default_auth_token;
        let mut hoisting = runtime_config
            .hoisting
            .unwrap_or(HoistingMode::SingleVersion);
        let mut link_backend = LinkBackend::Auto;
        let mut strict_peers = false;
        let mut frozen_lockfile_default = false;
        let mut registry_concurrency = 128;
        let mut default_registry_auth_scheme = AuthScheme::Bearer;
        let mut always_auth = runtime_config.always_auth;

        apply_default_registry_env(&mut default_registry, &mut default_registry_auth_token);
        apply_auth_env(
            &mut default_registry_auth_token,
            &mut default_registry_auth_scheme,
        );
        apply_install_env(
            &mut hoisting,
            &mut link_backend,
            &mut strict_peers,
            &mut frozen_lockfile_default,
            &mut registry_concurrency,
            &mut always_auth,
        );

        let (verbose, log_file) = read_logging_env();

        SnpmConfig {
            cache_dir,
            data_dir,
            allow_scripts,
            min_package_age_days,
            min_package_cache_age_days,
            default_registry,
            scoped_registries,
            registry_auth,
            default_registry_auth_token,
            default_registry_auth_scheme: if runtime_config_default_auth_basic {
                AuthScheme::Basic
            } else {
                default_registry_auth_scheme
            },
            registry_auth_schemes,
            hoisting,
            link_backend,
            strict_peers,
            frozen_lockfile_default,
            always_auth,
            registry_concurrency,
            verbose,
            log_file,
        }
    }
}
