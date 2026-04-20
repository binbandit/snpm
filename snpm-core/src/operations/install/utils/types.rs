use crate::resolve::{PackageId, ResolutionGraph, ResolvedPackage};
use std::collections::BTreeMap;
use std::path::PathBuf;

use std::env;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallScenario {
    Hot,
    WarmLinkOnly,
    WarmPartialCache,
    Cold,
}

pub struct CacheCheckResult {
    pub cached: BTreeMap<PackageId, PathBuf>,
    pub missing: Vec<ResolvedPackage>,
}

#[derive(Debug, Clone)]
pub struct InstallOptions {
    pub requested: Vec<String>,
    pub dev: bool,
    pub include_dev: bool,
    pub frozen_lockfile: FrozenLockfileMode,
    pub strict_no_lockfile: bool,
    pub force: bool,
    pub silent_summary: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrozenLockfileOverride {
    /// `--frozen-lockfile`
    Frozen,
    /// `--no-frozen-lockfile`
    No,
    /// `--prefer-frozen-lockfile`
    Prefer,
    /// `--fix-lockfile`
    Fix,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrozenLockfileMode {
    /// Fail when lockfile state is missing or out of sync.
    Frozen,
    /// Prefer lockfile reuse when it matches; otherwise fall back to fresh resolve.
    Prefer,
    /// Ignore lockfile data and always re-resolve.
    No,
    /// Re-resolve using existing lockfile constraints where possible.
    Fix,
}

impl FrozenLockfileMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Frozen => "frozen",
            Self::Prefer => "prefer",
            Self::No => "off",
            Self::Fix => "fix",
        }
    }

    pub fn from_override(
        override_mode: Option<FrozenLockfileOverride>,
        frozen_lockfile_default: bool,
    ) -> Self {
        match override_mode {
            Some(FrozenLockfileOverride::Frozen) => Self::Frozen,
            Some(FrozenLockfileOverride::No) => Self::No,
            Some(FrozenLockfileOverride::Prefer) => Self::Prefer,
            Some(FrozenLockfileOverride::Fix) => Self::Fix,
            None if frozen_lockfile_default || env::var_os("CI").is_some() => Self::Frozen,
            None => Self::Prefer,
        }
    }

    pub fn from_config_default(frozen_lockfile_default: bool) -> Self {
        Self::from_override(None, frozen_lockfile_default)
    }

    pub fn is_strict(&self) -> bool {
        matches!(self, Self::Frozen)
    }

    pub fn is_restrictive(&self) -> bool {
        match self {
            Self::No => false,
            Self::Fix | Self::Prefer | Self::Frozen => true,
        }
    }
}

impl FrozenLockfileOverride {
    pub fn from_flags(
        frozen_lockfile: bool,
        no_frozen_lockfile: bool,
        prefer_frozen_lockfile: bool,
        fix_lockfile: bool,
    ) -> Option<Self> {
        if frozen_lockfile {
            Some(Self::Frozen)
        } else if no_frozen_lockfile {
            Some(Self::No)
        } else if prefer_frozen_lockfile {
            Some(Self::Prefer)
        } else if fix_lockfile {
            Some(Self::Fix)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_override_prefers_explicit_lockfile_mode() {
        let fix = FrozenLockfileMode::from_override(Some(FrozenLockfileOverride::Fix), false);
        assert!(matches!(fix, FrozenLockfileMode::Fix));
    }

    #[test]
    fn from_override_prefers_default_frozen_in_ci_or_env() {
        let frozen = FrozenLockfileMode::from_override(None, true);
        assert!(matches!(frozen, FrozenLockfileMode::Frozen));
    }

    #[test]
    fn from_override_defaults_to_prefer() {
        let prefer = FrozenLockfileMode::from_override(None, false);
        assert!(matches!(prefer, FrozenLockfileMode::Prefer));
    }

    #[test]
    fn from_flags_maps_fix_option() {
        let mapped = FrozenLockfileOverride::from_flags(false, false, false, true);
        assert!(matches!(mapped, Some(FrozenLockfileOverride::Fix)));
    }
}

#[derive(Debug, Clone)]
pub struct InstallResult {
    pub package_count: usize,
    pub elapsed_seconds: f32,
}

#[derive(Debug)]
pub struct OutdatedEntry {
    pub name: String,
    pub current: Option<String>,
    pub wanted: String,
}

#[derive(Debug, Clone)]
pub struct ParsedSpec {
    pub name: String,
    pub range: String,
    pub protocol: Option<String>,
}

pub struct ScenarioResult {
    pub scenario: InstallScenario,
    pub cache_check: Option<CacheCheckResult>,
    pub graph: Option<ResolutionGraph>,
    pub integrity_state: Option<IntegrityState>,
}

impl ScenarioResult {
    pub fn cold() -> Self {
        Self {
            scenario: InstallScenario::Cold,
            cache_check: None,
            graph: None,
            integrity_state: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct IntegrityState {
    pub lockfile_hash: String,
    pub patch_hash: String,
}
