use std::sync::OnceLock;

use snpm_core::{
    SnpmConfig,
    operations::install::{FrozenLockfileMode, FrozenLockfileOverride},
};

static GLOBAL_FROZEN: OnceLock<Option<FrozenLockfileOverride>> = OnceLock::new();

#[derive(Clone, Copy)]
pub(crate) struct FrozenLockfileResolution {
    pub(crate) mode: FrozenLockfileMode,
    pub(crate) strict_no_lockfile: bool,
}

pub(crate) fn set_global_frozen_override(mode: Option<FrozenLockfileOverride>) {
    let _ = GLOBAL_FROZEN.set(mode);
}

pub(crate) fn global_frozen_override() -> Option<FrozenLockfileOverride> {
    GLOBAL_FROZEN.get().copied().unwrap_or_default()
}

pub(crate) fn resolve_frozen_lockfile_mode(
    config: &SnpmConfig,
    override_mode: Option<FrozenLockfileOverride>,
) -> FrozenLockfileResolution {
    let effective_override = override_mode;
    let mode =
        FrozenLockfileMode::from_override(effective_override, config.frozen_lockfile_default);
    let strict_no_lockfile = matches!(effective_override, Some(FrozenLockfileOverride::Frozen));

    FrozenLockfileResolution {
        mode,
        strict_no_lockfile,
    }
}

pub(crate) fn resolve_frozen_lockfile_mode_for_flags(
    config: &SnpmConfig,
    frozen_lockfile: bool,
    no_frozen_lockfile: bool,
    prefer_frozen_lockfile: bool,
    fix_lockfile: bool,
    force: bool,
) -> FrozenLockfileResolution {
    let command_override = FrozenLockfileOverride::from_flags(
        frozen_lockfile,
        no_frozen_lockfile,
        prefer_frozen_lockfile,
        fix_lockfile,
    );
    let command_override =
        if command_override.is_none() && force && global_frozen_override().is_none() {
            Some(FrozenLockfileOverride::No)
        } else {
            command_override
        };
    let effective_override = command_override.or(global_frozen_override());

    resolve_frozen_lockfile_mode(config, effective_override)
}

pub(crate) fn frozen_override_from_cli(
    frozen_lockfile: bool,
    no_frozen_lockfile: bool,
    prefer_frozen_lockfile: bool,
) -> Option<FrozenLockfileOverride> {
    if frozen_lockfile {
        Some(FrozenLockfileOverride::Frozen)
    } else if no_frozen_lockfile {
        Some(FrozenLockfileOverride::No)
    } else if prefer_frozen_lockfile {
        Some(FrozenLockfileOverride::Prefer)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snpm_core::config::{
        AuthScheme, HoistingMode, LinkBackend, SnpmConfig,
    };
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::Path;

    fn make_config(frozen_lockfile_default: bool) -> SnpmConfig {
        SnpmConfig {
            cache_dir: Path::new("/tmp/snpm-cache").to_path_buf(),
            data_dir: Path::new("/tmp/snpm-data").to_path_buf(),
            allow_scripts: BTreeSet::new(),
            min_package_age_days: None,
            min_package_cache_age_days: None,
            default_registry: "https://registry.npmjs.org".to_string(),
            scoped_registries: BTreeMap::new(),
            registry_auth: BTreeMap::new(),
            default_registry_auth_token: None,
            default_registry_auth_scheme: AuthScheme::Bearer,
            registry_auth_schemes: BTreeMap::new(),
            hoisting: HoistingMode::SingleVersion,
            link_backend: LinkBackend::Auto,
            strict_peers: false,
            frozen_lockfile_default,
            always_auth: false,
            registry_concurrency: 64,
            verbose: false,
            log_file: None,
        }
    }

    #[test]
    fn resolve_frozen_lockfile_mode_for_flags_respects_fix_flag() {
        let config = make_config(false);
        let resolved = resolve_frozen_lockfile_mode_for_flags(&config, false, false, false, true, false);

        assert!(matches!(resolved.mode, FrozenLockfileMode::Fix));
        assert!(!resolved.strict_no_lockfile);
    }

    #[test]
    fn resolve_frozen_lockfile_mode_for_flags_force_sets_no_lockfile_without_cli_override() {
        let config = make_config(false);
        let resolved = resolve_frozen_lockfile_mode_for_flags(&config, false, false, false, false, true);

        assert!(matches!(resolved.mode, FrozenLockfileMode::No));
        assert!(!resolved.strict_no_lockfile);
    }

    #[test]
    fn resolve_frozen_lockfile_mode_for_flags_uses_config_default_without_cli_override() {
        let config = make_config(true);
        let resolved = resolve_frozen_lockfile_mode_for_flags(&config, false, false, false, false, false);

        assert!(matches!(resolved.mode, FrozenLockfileMode::Frozen));
        assert!(!resolved.strict_no_lockfile);
    }

    #[test]
    fn resolve_frozen_lockfile_mode_for_flags_prefers_preferred_when_no_force() {
        let config = make_config(false);
        let resolved = resolve_frozen_lockfile_mode_for_flags(&config, false, false, true, false, false);

        assert!(matches!(resolved.mode, FrozenLockfileMode::Prefer));
    }

    #[test]
    fn resolve_frozen_lockfile_mode_for_flags_resolve_override_precedence() {
        let frozen = FrozenLockfileOverride::from_flags(true, false, false, false);
        let no = FrozenLockfileOverride::from_flags(false, true, false, false);
        let prefer = FrozenLockfileOverride::from_flags(false, false, true, false);
        let fix = FrozenLockfileOverride::from_flags(false, false, false, true);
        let none = FrozenLockfileOverride::from_flags(false, false, false, false);

        assert!(matches!(frozen, Some(FrozenLockfileOverride::Frozen)));
        assert!(matches!(no, Some(FrozenLockfileOverride::No)));
        assert!(matches!(prefer, Some(FrozenLockfileOverride::Prefer)));
        assert!(matches!(fix, Some(FrozenLockfileOverride::Fix)));
        assert!(none.is_none());
    }
}
