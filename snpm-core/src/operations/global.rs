use crate::linker::bins::link_bins;
use crate::linker::fs::link_dir;
use crate::resolve;
use crate::store;
use crate::{Result, SnpmConfig, SnpmError, console};
use reqwest::Client;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

pub async fn install_global(config: &SnpmConfig, packages: Vec<String>) -> Result<()> {
    if packages.is_empty() {
        return Ok(());
    }

    let client = Client::new();
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
        let (name, range) = parse_spec(spec);
        console::step(&format!("Installing {} globally", name));

        let mut root_deps = BTreeMap::new();
        root_deps.insert(name.clone(), range.clone());

        let graph = resolve::resolve(
            config,
            &client,
            &root_deps,
            &BTreeMap::new(),
            config.min_package_age_days,
            false,
            None,
            |_| async { Ok(()) },
        )
        .await?;

        let root_dep = graph
            .root
            .dependencies
            .get(&name)
            .ok_or_else(|| SnpmError::ResolutionFailed {
                name: name.clone(),
                range: range.clone(),
                reason: "package not found in resolution".into(),
            })?;

        let package = graph
            .packages
            .get(&root_dep.resolved)
            .ok_or_else(|| SnpmError::ResolutionFailed {
                name: name.clone(),
                range: range.clone(),
                reason: "resolved package missing from graph".into(),
            })?;

        let store_path = store::ensure_package(config, package, &client).await?;

        let package_dir = global_dir.join(&name);
        if package_dir.exists() {
            fs::remove_dir_all(&package_dir).ok();
        }

        link_dir(config, &store_path, &package_dir)?;
        link_bins(&package_dir, &global_bin_dir, &name)?;

        console::added(&name, &root_dep.resolved.version, false);
    }

    println!();
    print_path_setup_hint(&global_bin_dir);

    Ok(())
}

fn print_path_setup_hint(bin_dir: &std::path::Path) {
    let bin_path = bin_dir.display();

    if std::env::var("PATH")
        .map(|p| p.contains(&bin_dir.to_string_lossy().to_string()))
        .unwrap_or(false)
    {
        console::info(&format!("Binaries available at: {}", bin_path));
        return;
    }

    console::info(&format!("Binaries installed to: {}", bin_path));
    println!();
    console::info("Add to PATH by running:");
    println!();

    let shell = std::env::var("SHELL").unwrap_or_default();

    if shell.contains("zsh") {
        println!("  echo 'export PATH=\"{}:$PATH\"' >> ~/.zshrc", bin_path);
        println!("  source ~/.zshrc");
    } else if shell.contains("bash") {
        println!("  echo 'export PATH=\"{}:$PATH\"' >> ~/.bashrc", bin_path);
        println!("  source ~/.bashrc");
    } else if shell.contains("fish") {
        println!("  fish_add_path {}", bin_path);
    } else {
        println!("  export PATH=\"{}:$PATH\"", bin_path);
    }
}

pub async fn remove_global(config: &SnpmConfig, packages: Vec<String>) -> Result<()> {
    if packages.is_empty() {
        return Ok(());
    }

    let global_dir = config.global_dir();
    let global_bin_dir = config.global_bin_dir();

    for spec in &packages {
        let (name, _) = parse_spec(spec);

        let package_dir = global_dir.join(&name);
        if package_dir.exists() {
            fs::remove_dir_all(&package_dir).map_err(|source| SnpmError::WriteFile {
                path: package_dir.clone(),
                source,
            })?;
        }

        remove_package_bins(&name, &global_bin_dir)?;
        console::removed(&name);
    }

    Ok(())
}

fn remove_package_bins(package_name: &str, bin_dir: &PathBuf) -> Result<()> {
    if !bin_dir.exists() {
        return Ok(());
    }

    let entries = fs::read_dir(bin_dir).map_err(|source| SnpmError::ReadFile {
        path: bin_dir.clone(),
        source,
    })?;

    for entry in entries.flatten() {
        let path = entry.path();

        if !path.is_symlink() {
            continue;
        }

        if let Ok(target) = fs::read_link(&path) {
            let target_str = target.to_string_lossy();
            if target_str.contains(package_name) {
                fs::remove_file(&path).ok();
            }
        }
    }

    Ok(())
}

fn parse_spec(spec: &str) -> (String, String) {
    if let Some(without_at) = spec.strip_prefix('@') {
        if let Some(index) = without_at.rfind('@') {
            let (scope_and_name, range) = without_at.split_at(index);
            let name = format!("@{}", scope_and_name);
            let requested = range.trim_start_matches('@').to_string();
            return (name, requested);
        } else {
            return (spec.to_string(), "latest".to_string());
        }
    }

    if let Some(index) = spec.rfind('@') {
        let (name, range) = spec.split_at(index);
        let requested = range.trim_start_matches('@').to_string();
        (name.to_string(), requested)
    } else {
        (spec.to_string(), "latest".to_string())
    }
}
