use crate::linker::bins::link_bins;
use crate::linker::fs::link_dir;
use crate::resolve;
use crate::store;
use crate::{Result, SnpmConfig, SnpmError, console, http};

use std::collections::BTreeMap;
use std::fs;

use super::super::install::manifest::parse_spec;
use super::shell::print_path_setup_hint;

pub async fn install_global(config: &SnpmConfig, packages: Vec<String>) -> Result<()> {
    if packages.is_empty() {
        return Ok(());
    }

    let client = http::create_client()?;
    let global_dir = config.global_dir();
    let global_bin_dir = config.global_bin_dir();

    fs::create_dir_all(&global_dir).map_err(|source| SnpmError::WriteFile {
        path: global_dir.clone(),
        source,
    })?;

    fs::create_dir_all(&global_bin_dir).map_err(|source| SnpmError::WriteFile {
        path: global_bin_dir.clone(),
        source,
    })?;

    for spec in &packages {
        install_package(config, &client, &global_dir, &global_bin_dir, spec).await?;
    }

    println!();
    print_path_setup_hint(&global_bin_dir);

    Ok(())
}

async fn install_package(
    config: &SnpmConfig,
    client: &reqwest::Client,
    global_dir: &std::path::Path,
    global_bin_dir: &std::path::Path,
    spec: &str,
) -> Result<()> {
    let (name, range) = parse_spec(spec);
    console::step(&format!("Installing {} globally", name));

    let mut root_deps = BTreeMap::new();
    root_deps.insert(name.clone(), range.clone());

    let graph = resolve::resolve(
        config,
        client,
        &root_deps,
        &BTreeMap::new(),
        config.min_package_age_days,
        false,
        None,
        None,
        |_| async { Ok(()) },
    )
    .await?;

    let root_dep =
        graph
            .root
            .dependencies
            .get(&name)
            .ok_or_else(|| SnpmError::ResolutionFailed {
                name: name.clone(),
                range: range.clone(),
                reason: "package not found in resolution".into(),
            })?;

    let package =
        graph
            .packages
            .get(&root_dep.resolved)
            .ok_or_else(|| SnpmError::ResolutionFailed {
                name: name.clone(),
                range: range.clone(),
                reason: "resolved package missing from graph".into(),
            })?;

    let store_path = store::ensure_package(config, package, client).await?;
    let package_dir = global_dir.join(&name);

    if package_dir.exists() {
        fs::remove_dir_all(&package_dir).ok();
    }

    link_dir(config, &store_path, &package_dir)?;
    link_bins(&package_dir, global_bin_dir, &name)?;

    console::added(&name, &root_dep.resolved.version, false);
    Ok(())
}
