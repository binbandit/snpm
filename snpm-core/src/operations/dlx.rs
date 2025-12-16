use crate::console;
use crate::linker;
use crate::resolve;
use crate::store;
use crate::{Project, Result, SnpmConfig, SnpmError};
use futures::lock::Mutex;
use reqwest::Client;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tempfile::TempDir;
use tokio::task::JoinHandle;

pub async fn dlx(config: &SnpmConfig, package_spec: String, args: Vec<String>) -> Result<()> {
    let temp_dir = TempDir::new().map_err(|e| SnpmError::Io {
        path: PathBuf::from("temp_dlx"),
        source: e,
    })?;
    let temp_path = temp_dir.path().to_path_buf();

    console::verbose(&format!(
        "dlx: executing {} in temporary directory {}",
        package_spec,
        temp_path.display()
    ));

    let (name, range) = parse_spec(&package_spec);
    let mut root_deps = BTreeMap::new();
    root_deps.insert(name.clone(), range.clone());

    let registry_client = Client::new();

    console::step("Resolving package for dlx");

    let store_paths = Arc::new(Mutex::new(BTreeMap::new()));
    let store_config = config.clone();
    let store_client = registry_client.clone();
    let store_tasks: Arc<Mutex<Vec<JoinHandle<Result<()>>>>> = Arc::new(Mutex::new(Vec::new()));

    let paths = store_paths.clone();
    let client = store_client.clone();
    let tasks = store_tasks.clone();

    // DLX runs in a temporary directory, so we must copy files to avoid
    // symlink resolution issues (Node.js preserving symlink paths).
    let mut dlx_config = config.clone();
    dlx_config.link_backend = crate::config::LinkBackend::Copy;
    let config = &dlx_config;
    // Note: store resolution uses the original config (store_config) correctly below.
    // Actually, ensure_package needs correct config too for cache/store location.
    // But LinkBackend is only used in linker. Store location is same.
    let cfg = store_config.clone();

    let progress_count = Arc::new(AtomicUsize::new(0));
    let progress_total = Arc::new(AtomicUsize::new(1));

    let root_protocols = BTreeMap::new();

    let graph = resolve::resolve(
        config,
        &registry_client,
        &root_deps,
        &root_protocols,
        config.min_package_age_days,
        true,
        None,
        move |package| {
            let cfg = cfg.clone();
            let client = client.clone();
            let paths = paths.clone();
            let tasks = tasks.clone();
            let count = progress_count.clone();
            let total = progress_total.clone();
            let name = package.id.name.clone();

            async move {
                let current = count.fetch_add(1, Ordering::Relaxed) + 1;
                let mut total_val = total.load(Ordering::Relaxed);
                if current > total_val {
                    total_val = current;
                    total.store(total_val, Ordering::Relaxed);
                }
                console::progress("🚚", &name, current, total_val);

                let package_id = package.id.clone();

                let handle = tokio::spawn(async move {
                    let path = store::ensure_package(&cfg, &package, &client).await?;
                    let mut map = paths.lock().await;
                    map.insert(package_id, path);
                    Ok::<(), SnpmError>(())
                });

                let mut guard = tasks.lock().await;
                guard.push(handle);

                Ok(())
            }
        },
    )
    .await?;

    {
        let handles = {
            let mut guard = store_tasks.lock().await;
            std::mem::take(&mut *guard)
        };

        for handle in handles {
            let result = handle.await.map_err(|error| SnpmError::StoreTask {
                reason: error.to_string(),
            })?;
            result?;
        }
    }

    let store_paths_map = {
        let guard = store_paths.lock().await;
        guard.clone()
    };

    console::step("Linking environment");

    let manifest = crate::project::Manifest {
        name: Some("dlx-project".to_string()),
        version: Some("0.0.0".to_string()),
        dependencies: root_deps,
        dev_dependencies: BTreeMap::new(),
        scripts: BTreeMap::new(),
        pnpm: None,
        snpm: None,
        workspaces: None,
    };

    let project = Project {
        root: temp_path.clone(),
        manifest_path: temp_path.join("package.json"),
        manifest,
    };

    linker::link(config, None, &project, &graph, &store_paths_map, false)?;

    let bin_name = if let Some(idx) = name.rfind('/') {
        &name[idx + 1..]
    } else {
        &name
    };

    let bin_path_dir = temp_path.join("node_modules").join(".bin");
    let bin_path = bin_path_dir.join(bin_name);

    if !bin_path.exists() {
        if let Ok(entries) = std::fs::read_dir(&bin_path_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() || path.is_symlink() {
                    console::verbose(&format!(
                        "Default binary {} not found, utilizing {}",
                        bin_name,
                        path.display()
                    ));
                    return run_bin(&path, args);
                }
            }
        }

        return Err(SnpmError::ScriptRun {
            name: name.clone(),
            reason: "Binary not found".to_string(),
        });
    }

    run_bin(&bin_path, args)
}

fn run_bin(bin_path: &PathBuf, args: Vec<String>) -> Result<()> {
    console::step(&format!("Running {}", bin_path.display()));

    let mut cmd = Command::new(bin_path);
    cmd.args(args);

    cmd.stdin(std::process::Stdio::inherit());
    cmd.stdout(std::process::Stdio::inherit());
    cmd.stderr(std::process::Stdio::inherit());

    let status = cmd.status().map_err(|e| SnpmError::ScriptRun {
        name: bin_path.to_string_lossy().to_string(),
        reason: e.to_string(),
    })?;

    if !status.success() {
        return Err(SnpmError::ScriptFailed {
            name: bin_path.to_string_lossy().to_string(),
            code: status.code().unwrap_or(-1),
        });
    }

    Ok(())
}

fn parse_spec(spec: &str) -> (String, String) {
    if let Some(without_at) = spec.strip_prefix('@') {
        if let Some(idx) = without_at.rfind('@') {
            let (scope_and_name, range) = without_at.split_at(idx);
            let name = format!("@{}", scope_and_name);
            let requested = range.trim_start_matches('@').to_string();
            return (name, requested);
        } else {
            return (spec.to_string(), "latest".to_string());
        }
    }

    if let Some(idx) = spec.rfind('@') {
        let (name, range) = spec.split_at(idx);
        let requested = range.trim_start_matches('@').to_string();
        (name.to_string(), requested)
    } else {
        (spec.to_string(), "latest".to_string())
    }
}
