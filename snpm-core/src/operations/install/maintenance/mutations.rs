use crate::console;
use crate::registry::RegistryProtocol;
use crate::{Project, Result, SnpmConfig};

use std::collections::BTreeMap;

use super::super::manifest::{is_special_protocol_spec, parse_spec};
use super::super::project_install::install;
use super::super::utils::{FrozenLockfileMode, InstallOptions};
use super::outdated::outdated;

pub async fn remove(
    config: &SnpmConfig,
    project: &mut Project,
    specs: Vec<String>,
    frozen_lockfile: FrozenLockfileMode,
    strict_no_lockfile: bool,
) -> Result<()> {
    if specs.is_empty() {
        return Ok(());
    }

    // Removing a dependency requires a lockfile update, which frozen
    // mode forbids. Refuse up front — mutating package.json first and
    // failing later would leave the manifest out of sync with the
    // lockfile and node_modules.
    if matches!(frozen_lockfile, FrozenLockfileMode::Frozen) {
        return Err(crate::SnpmError::Internal {
            reason: "cannot remove packages when using frozen-lockfile".to_string(),
        });
    }

    let mut manifest = project.manifest.clone();

    for spec in specs {
        let (name, _) = parse_spec(&spec);
        let removed_runtime = manifest.dependencies.remove(&name).is_some();
        let removed_dev = manifest.dev_dependencies.remove(&name).is_some();
        let removed_optional = manifest.optional_dependencies.remove(&name).is_some();

        if removed_runtime || removed_dev || removed_optional {
            console::removed(&name);
        } else {
            console::warn(&format!("{name} is not a dependency of this project"));
        }
    }

    project.write_manifest(&manifest)?;
    project.manifest = manifest;

    reinstall(
        config,
        project,
        true,
        frozen_lockfile,
        strict_no_lockfile,
        false,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn upgrade(
    config: &SnpmConfig,
    project: &mut Project,
    packages: Vec<String>,
    frozen_lockfile: FrozenLockfileMode,
    strict_no_lockfile: bool,
    production: bool,
    force: bool,
    latest: bool,
) -> Result<()> {
    let include_dev = !production;

    // `--latest` bumps manifest ranges to each dependency's newest
    // published version (beyond the current range), so it can't reuse the
    // in-range `outdated` report — a dep already at the newest in-range
    // version is filtered out of that report but may still trail `latest`.
    if latest {
        return upgrade_to_latest(
            config,
            project,
            packages,
            frozen_lockfile,
            strict_no_lockfile,
            production,
            force,
        )
        .await;
    }

    if packages.is_empty() {
        return reinstall(
            config,
            project,
            include_dev,
            frozen_lockfile,
            strict_no_lockfile,
            force,
        )
        .await;
    }

    // Same ordering concern as `remove`: fail before touching the
    // manifest when the lockfile is frozen.
    if matches!(frozen_lockfile, FrozenLockfileMode::Frozen) {
        return Err(crate::SnpmError::Internal {
            reason: "cannot upgrade packages when using frozen-lockfile".to_string(),
        });
    }

    // outdated() also reports deps whose installed version already
    // satisfies the range but have a newer `latest` beyond it (that is
    // what the Latest column shows). A plain upgrade has nothing to do
    // for those — without this filter it would rewrite the identical
    // spec, print a false "updating", and reinstall for nothing.
    let entries: Vec<_> = outdated(config, project, include_dev, force)
        .await?
        .into_iter()
        .filter(|entry| entry.current.as_deref() != Some(entry.wanted.as_str()))
        .collect();
    if entries.is_empty() {
        return Ok(());
    }

    let wanted_by_name = wanted_versions(entries);
    let mut manifest = project.manifest.clone();
    let mut changed = false;

    for spec in packages {
        let (name, _) = parse_spec(&spec);
        let Some(wanted) = wanted_by_name.get(&name) else {
            continue;
        };

        let updated = update_manifest_entry(&mut manifest, &name, wanted, production);
        changed |= updated;
    }

    if !changed {
        return Ok(());
    }

    project.write_manifest(&manifest)?;
    project.manifest = manifest;

    reinstall(
        config,
        project,
        include_dev,
        frozen_lockfile,
        strict_no_lockfile,
        force,
    )
    .await
}

/// Upgrade the manifest ranges of the target dependencies (or every
/// upgradable dependency when none are named) to each package's newest
/// published version, then reinstall.
#[allow(clippy::too_many_arguments)]
async fn upgrade_to_latest(
    config: &SnpmConfig,
    project: &mut Project,
    packages: Vec<String>,
    frozen_lockfile: FrozenLockfileMode,
    strict_no_lockfile: bool,
    production: bool,
    force: bool,
) -> Result<()> {
    if matches!(frozen_lockfile, FrozenLockfileMode::Frozen) {
        return Err(crate::SnpmError::Internal {
            reason: "cannot upgrade packages when using frozen-lockfile".to_string(),
        });
    }

    let include_dev = !production;
    let target_names = if packages.is_empty() {
        collect_upgradable_names(&project.manifest, include_dev)
    } else {
        packages.iter().map(|spec| parse_spec(spec).0).collect()
    };

    let client = crate::http::create_client()?;
    let npm = RegistryProtocol::npm();
    let mut manifest = project.manifest.clone();
    let mut changed = false;

    for name in target_names {
        // Skip deps snpm can't range-resolve (git/file/link/workspace/…):
        // rewriting them to a registry version would break the reference.
        if current_spec(&manifest, &name).is_none_or(is_registry_unresolvable_spec) {
            continue;
        }

        match crate::registry::fetch_package(config, &client, &name, &npm).await {
            Ok(package) => {
                // Route the dist-tag through select_version so the
                // configured minimum package age applies here too — writing
                // a too-young version to the manifest would make the
                // subsequent reinstall fail *after* the rewrite.
                match crate::version::select_version(
                    &name,
                    "latest",
                    &package,
                    config.min_package_age_days,
                    force,
                ) {
                    Ok(meta) => {
                        changed |=
                            update_manifest_entry(&mut manifest, &name, &meta.version, production);
                    }
                    Err(error) => {
                        console::warn(&format!("skipping {name}: {error}"));
                    }
                }
            }
            Err(error) => {
                console::warn(&format!("could not fetch latest for {name}: {error}"));
            }
        }
    }

    if changed {
        project.write_manifest(&manifest)?;
        project.manifest = manifest;
    }

    reinstall(
        config,
        project,
        include_dev,
        frozen_lockfile,
        strict_no_lockfile,
        force,
    )
    .await
}

/// The names of manifest dependencies eligible for `--latest`: runtime and
/// optional deps always, dev deps unless production-only.
fn collect_upgradable_names(manifest: &crate::project::Manifest, include_dev: bool) -> Vec<String> {
    let mut names: Vec<String> = manifest
        .dependencies
        .keys()
        .chain(manifest.optional_dependencies.keys())
        .cloned()
        .collect();
    if include_dev {
        names.extend(manifest.dev_dependencies.keys().cloned());
    }
    names.sort();
    names.dedup();
    names
}

fn current_spec<'a>(manifest: &'a crate::project::Manifest, name: &str) -> Option<&'a str> {
    manifest
        .dependencies
        .get(name)
        .or_else(|| manifest.dev_dependencies.get(name))
        .or_else(|| manifest.optional_dependencies.get(name))
        .map(String::as_str)
}

/// Specs whose target isn't a plain registry range: special protocols
/// (git/link/workspace/npm-alias/…) plus `file:`, which
/// `is_special_protocol_spec` doesn't cover.
fn is_registry_unresolvable_spec(spec: &str) -> bool {
    is_special_protocol_spec(spec) || spec.starts_with("file:")
}

fn wanted_versions(entries: Vec<super::super::utils::OutdatedEntry>) -> BTreeMap<String, String> {
    let mut wanted_by_name = BTreeMap::new();
    for entry in entries {
        wanted_by_name.insert(entry.name, entry.wanted);
    }
    wanted_by_name
}

fn update_manifest_entry(
    manifest: &mut crate::project::Manifest,
    name: &str,
    wanted: &str,
    production: bool,
) -> bool {
    let mut updated = false;

    if let Some(current) = manifest.dependencies.get_mut(name)
        && !is_special_protocol_spec(current)
    {
        let next = rewrite_spec_preserving_operator(current, wanted);
        console::info(&format!("updating {name} to {next}"));
        *current = next;
        updated = true;
    }

    if !updated
        && !production
        && let Some(current) = manifest.dev_dependencies.get_mut(name)
        && !is_special_protocol_spec(current)
    {
        let next = rewrite_spec_preserving_operator(current, wanted);
        console::info(&format!("updating {name} (dev) to {next}"));
        *current = next;
        updated = true;
    }

    // Optional deps install in production too, so they are not gated on
    // the production flag.
    if !updated
        && let Some(current) = manifest.optional_dependencies.get_mut(name)
        && !is_special_protocol_spec(current)
    {
        let next = rewrite_spec_preserving_operator(current, wanted);
        console::info(&format!("updating {name} (optional) to {next}"));
        *current = next;
        updated = true;
    }

    updated
}

/// Keep the range operator the user chose: `~1.2.0` upgrades to
/// `~<wanted>`, an exact pin upgrades to the exact new version, and only
/// caret (or complex) ranges get the caret default. Blindly writing
/// `^<wanted>` would silently widen a tilde or exact constraint to all
/// of the new major.
fn rewrite_spec_preserving_operator(current: &str, wanted: &str) -> String {
    let current = current.trim();
    if let Some(rest) = current.strip_prefix('~')
        && !rest.is_empty()
    {
        return format!("~{wanted}");
    }
    if is_plain_version(current) {
        return wanted.to_string();
    }
    format!("^{wanted}")
}

fn is_plain_version(spec: &str) -> bool {
    let bare = spec.strip_prefix('v').unwrap_or(spec);
    !bare.is_empty()
        && bare.chars().next().is_some_and(|c| c.is_ascii_digit())
        && bare
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+'))
        && bare
            .split(['-', '+'])
            .next()
            .unwrap_or("")
            .matches('.')
            .count()
            == 2
}

async fn reinstall(
    config: &SnpmConfig,
    project: &mut Project,
    include_dev: bool,
    frozen_lockfile: FrozenLockfileMode,
    strict_no_lockfile: bool,
    force: bool,
) -> Result<()> {
    install(
        config,
        project,
        InstallOptions {
            requested: Vec::new(),
            dev: false,
            include_dev,
            frozen_lockfile,
            strict_no_lockfile,
            force,
            silent_summary: false,
        },
    )
    .await?;

    Ok(())
}
