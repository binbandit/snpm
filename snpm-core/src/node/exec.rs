use crate::{Result, SnpmConfig};

use super::aliases;
use super::current;
use super::discover::{self, PinnedNode, PinnedNodeSource};
use super::install::{self, InstallSummary};
use super::resolve::{self, ResolvedNodeVersion};
use super::uninstall::is_version_installed;

use std::path::{Path, PathBuf};

pub const BIN_OVERRIDE_ENV: &str = "SNPM_NODE_BIN_OVERRIDE";

pub fn node_bin_dir_for_subprocess(project_root: &Path) -> Option<PathBuf> {
    if let Ok(value) = std::env::var(BIN_OVERRIDE_ENV) {
        let path = PathBuf::from(value.trim());
        if !path.as_os_str().is_empty() {
            return Some(path);
        }
    }

    if !auto_switch_enabled() {
        return None;
    }

    let config = SnpmConfig::from_env();
    active_for_project_offline(&config, project_root)
        .ok()
        .flatten()
        .map(|active| active.bin_dir)
}

#[derive(Debug, Clone)]
pub struct ActiveNode {
    pub version: String,
    pub bin_dir: PathBuf,
    pub source: ActiveNodeSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActiveNodeSource {
    Pin(PinnedNodeSource),
    Default,
    Current,
}

pub fn auto_switch_enabled() -> bool {
    match std::env::var("SNPM_NODE_AUTO") {
        Err(_) => true,
        Ok(value) => !matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "0" | "false" | "no" | "off"
        ),
    }
}

pub fn auto_install_enabled() -> bool {
    match std::env::var("SNPM_NODE_AUTO_INSTALL") {
        Err(_) => true,
        Ok(value) => !matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "0" | "false" | "no" | "off"
        ),
    }
}

pub fn active_for_project_offline(
    config: &SnpmConfig,
    project_root: &Path,
) -> Result<Option<ActiveNode>> {
    let pinned = discover::discover_pinned(project_root)?;
    resolve_active_offline(config, pinned)
}

pub async fn ensure_active_for_project(
    config: &SnpmConfig,
    project_root: &Path,
) -> Result<Option<ActiveNode>> {
    let pinned = discover::discover_pinned(project_root)?;
    let Some(pin) = pinned else {
        return resolve_default(config);
    };

    let resolved = resolve::resolve_spec(config, &pin.spec, true).await?;
    let normalized = resolved.normalized.clone();

    if !is_version_installed(config, &normalized) && auto_install_enabled() {
        install::install_version(config, &normalized).await?;
    } else if !is_version_installed(config, &normalized) {
        return Ok(None);
    }

    Ok(Some(ActiveNode {
        version: normalized.clone(),
        bin_dir: config.node_version_bin_dir(&normalized),
        source: ActiveNodeSource::Pin(pin.source),
    }))
}

/// Make sure the project's pinned Node version is installed before a
/// script/exec spawns, so the synchronous PATH construction can find
/// it. Missing pinned versions are downloaded on demand as documented;
/// `SNPM_NODE_AUTO_INSTALL=0` turns that into a hard error instead of
/// silently running the system Node against the wrong pin.
pub async fn prepare_node_for_project(config: &SnpmConfig, project_root: &Path) -> Result<()> {
    let override_set = std::env::var(BIN_OVERRIDE_ENV).is_ok_and(|value| !value.trim().is_empty());
    if override_set || !auto_switch_enabled() {
        return Ok(());
    }

    let Some(pin) = discover::discover_pinned(project_root)? else {
        return Ok(());
    };

    // Satisfied by something already installed — nothing to download.
    if match_pin_offline(config, &pin)?.is_some() {
        return Ok(());
    }

    // engines.node declares a compatibility range, not a version
    // request: when no managed Node matches but the Node already on
    // PATH satisfies the range, leave the environment alone instead of
    // downloading a fresh release and silently switching the script's
    // runtime (or hard-failing offline).
    if matches!(pin.source, PinnedNodeSource::EnginesNode(_)) && system_node_satisfies(&pin.spec) {
        return Ok(());
    }

    match ensure_active_for_project(config, project_root).await? {
        Some(_) => Ok(()),
        None => Err(crate::SnpmError::Internal {
            reason: format!(
                "Node {} is pinned by this project but not installed, and auto-install \
                 is disabled (SNPM_NODE_AUTO_INSTALL=0). Run `snpm node install {}` first.",
                pin.spec, pin.spec
            ),
        }),
    }
}

pub async fn ensure_installed_for_spec(
    config: &SnpmConfig,
    spec: &str,
) -> Result<(ResolvedNodeVersion, InstallSummary)> {
    let resolved = resolve::resolve_spec(config, spec, true).await?;
    let summary = install::install_version(config, &resolved.normalized).await?;
    Ok((resolved, summary))
}

fn resolve_active_offline(
    config: &SnpmConfig,
    pinned: Option<PinnedNode>,
) -> Result<Option<ActiveNode>> {
    if let Some(pin) = pinned {
        if let Some(active) = match_pin_offline(config, &pin)? {
            return Ok(Some(active));
        }
        return Ok(None);
    }

    resolve_default(config)
}

fn match_pin_offline(config: &SnpmConfig, pin: &PinnedNode) -> Result<Option<ActiveNode>> {
    if let Some(normalized) = resolve::normalize_version(&pin.spec) {
        if is_version_installed(config, &normalized) {
            return Ok(Some(active_from(
                config,
                &normalized,
                ActiveNodeSource::Pin(pin.source.clone()),
            )));
        }
        return Ok(None);
    }

    if let Some(target) = aliases::read_alias(config, &pin.spec)?
        && let Some(normalized) = resolve::normalize_version(&target)
        && is_version_installed(config, &normalized)
    {
        return Ok(Some(active_from(
            config,
            &normalized,
            ActiveNodeSource::Pin(pin.source.clone()),
        )));
    }

    // Partial pins (".nvmrc" containing `20` or `20.10`) and
    // engines.node ranges (`>=18 <21`, `^20`) are not exact versions;
    // match them against what's already installed and take the newest
    // satisfying version.
    if let Some(normalized) = newest_installed_matching(config, &pin.spec) {
        return Ok(Some(active_from(
            config,
            &normalized,
            ActiveNodeSource::Pin(pin.source.clone()),
        )));
    }

    Ok(None)
}

/// Whether the `node` currently on PATH satisfies `spec`. False when no
/// system Node exists or the spec is not a parseable range — callers
/// fall through to auto-install in that case.
fn system_node_satisfies(spec: &str) -> bool {
    let Ok(range) = snpm_semver::RangeSet::parse(spec) else {
        return false;
    };
    let Ok(output) = std::process::Command::new("node").arg("--version").output() else {
        return false;
    };
    if !output.status.success() {
        return false;
    }

    let version = String::from_utf8_lossy(&output.stdout);
    let bare = version.trim().trim_start_matches('v');
    snpm_semver::Version::parse(bare)
        .map(|parsed| range.matches(&parsed))
        .unwrap_or(false)
}

fn newest_installed_matching(config: &SnpmConfig, spec: &str) -> Option<String> {
    let range = snpm_semver::RangeSet::parse(spec).ok()?;
    let installed = super::uninstall::list_installed_versions(config).ok()?;
    installed
        .into_iter()
        .filter_map(|version| {
            let bare = version.strip_prefix('v').unwrap_or(&version);
            snpm_semver::Version::parse(bare)
                .ok()
                .filter(|parsed| range.matches(parsed))
                .map(|parsed| (parsed, version.clone()))
        })
        .max_by(|left, right| left.0.cmp(&right.0))
        .map(|(_, version)| version)
}

fn resolve_default(config: &SnpmConfig) -> Result<Option<ActiveNode>> {
    if let Some(version) = current::read_current(config)?
        && is_version_installed(config, &version)
    {
        return Ok(Some(active_from(
            config,
            &version,
            ActiveNodeSource::Current,
        )));
    }

    if let Some(target) = aliases::read_alias(config, aliases::default_alias_name())?
        && let Some(normalized) = resolve::normalize_version(&target)
        && is_version_installed(config, &normalized)
    {
        return Ok(Some(active_from(
            config,
            &normalized,
            ActiveNodeSource::Default,
        )));
    }

    Ok(None)
}

fn active_from(config: &SnpmConfig, version: &str, source: ActiveNodeSource) -> ActiveNode {
    ActiveNode {
        version: version.to_string(),
        bin_dir: config.node_version_bin_dir(version),
        source,
    }
}
