use crate::console;
use crate::{Project, Result, SnpmConfig};

use std::collections::BTreeMap;

use super::super::manifest::{is_special_protocol_spec, parse_spec};
use super::super::project_install::install;
use super::super::utils::InstallOptions;
use super::outdated::outdated;

pub async fn remove(config: &SnpmConfig, project: &mut Project, specs: Vec<String>) -> Result<()> {
    if specs.is_empty() {
        return Ok(());
    }

    let mut manifest = project.manifest.clone();

    for spec in specs {
        let (name, _) = parse_spec(&spec);
        let removed_runtime = manifest.dependencies.remove(&name).is_some();
        let removed_dev = manifest.dev_dependencies.remove(&name).is_some();

        if removed_runtime || removed_dev {
            console::removed(&name);
        }
    }

    project.write_manifest(&manifest)?;
    project.manifest = manifest;

    reinstall(config, project, true, false).await
}

pub async fn upgrade(
    config: &SnpmConfig,
    project: &mut Project,
    packages: Vec<String>,
    production: bool,
    force: bool,
) -> Result<()> {
    let include_dev = !production;

    if packages.is_empty() {
        return reinstall(config, project, include_dev, force).await;
    }

    let entries = outdated(config, project, include_dev, force).await?;
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

    reinstall(config, project, include_dev, force).await
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
        *current = format!("^{}", wanted);
        console::info(&format!("updating {name} to ^{wanted}"));
        updated = true;
    }

    if !updated
        && !production
        && let Some(current) = manifest.dev_dependencies.get_mut(name)
        && !is_special_protocol_spec(current)
    {
        *current = format!("^{}", wanted);
        console::info(&format!("updating {name} (dev) to ^{wanted}"));
        updated = true;
    }

    updated
}

async fn reinstall(
    config: &SnpmConfig,
    project: &mut Project,
    include_dev: bool,
    force: bool,
) -> Result<()> {
    install(
        config,
        project,
        InstallOptions {
            requested: Vec::new(),
            dev: false,
            include_dev,
            frozen_lockfile: false,
            force,
            silent_summary: false,
        },
    )
    .await?;

    Ok(())
}
