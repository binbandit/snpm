use super::super::policy::is_dep_script_allowed;
use super::cache::{SideEffectsCacheEntry, SideEffectsCacheRestore};
use super::execute::run_present_scripts;
use super::manifest::{package_name, package_scripts, package_version, read_manifest};
use crate::console;
use crate::{Result, SnpmConfig, SnpmError, Workspace};

use rayon::prelude::*;
use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

const LIFECYCLE_SCRIPT_NAMES: [&str; 4] = ["preinstall", "install", "postinstall", "prepare"];

/// A dependency that needs its lifecycle scripts executed. Built during the
/// (serial) walk of node_modules and then handed to a parallel runner.
///
/// The `cache_entry` is the side-effects cache handle built from the pre-script
/// state of the package dir. We carry it into the job so the post-script
/// `save()` writes to the same path the next install's `restore_if_available`
/// will look in.
struct DepScriptJob {
    name: String,
    pkg_root: PathBuf,
    scripts: serde_json::Map<String, serde_json::Value>,
    cache_entry: Option<SideEffectsCacheEntry>,
}

pub fn run_install_scripts(
    config: &SnpmConfig,
    workspace: Option<&Workspace>,
    project_root: &Path,
) -> Result<Vec<String>> {
    run_install_scripts_for_projects(config, workspace, &[project_root])
}

pub fn run_install_scripts_for_projects(
    config: &SnpmConfig,
    workspace: Option<&Workspace>,
    project_roots: &[&Path],
) -> Result<Vec<String>> {
    let mut blocked = Vec::new();
    let mut blocked_seen = BTreeSet::new();
    let mut visited_dirs = BTreeSet::<PathBuf>::new();
    let mut jobs: Vec<DepScriptJob> = Vec::new();

    for project_root in project_roots {
        let node_modules = project_root.join("node_modules");

        if node_modules.is_dir() {
            walk_node_modules(
                config,
                workspace,
                &node_modules,
                &mut jobs,
                &mut blocked,
                &mut blocked_seen,
                &mut visited_dirs,
            )?;
        }
    }

    run_jobs(jobs)?;

    Ok(blocked)
}

fn walk_node_modules(
    config: &SnpmConfig,
    workspace: Option<&Workspace>,
    dir: &Path,
    jobs: &mut Vec<DepScriptJob>,
    blocked: &mut Vec<String>,
    blocked_seen: &mut BTreeSet<String>,
    visited_dirs: &mut BTreeSet<PathBuf>,
) -> Result<()> {
    let scan_dir = fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    if !visited_dirs.insert(scan_dir.clone()) {
        return Ok(());
    }

    for entry in fs::read_dir(&scan_dir).map_err(|source| SnpmError::ReadFile {
        path: scan_dir.clone(),
        source,
    })? {
        let entry = entry.map_err(|source| SnpmError::ReadFile {
            path: scan_dir.clone(),
            source,
        })?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        if entry.file_name() == OsStr::new(".bin") {
            continue;
        }

        let visit_path = fs::canonicalize(&path).unwrap_or(path.clone());
        let manifest_path = visit_path.join("package.json");

        if manifest_path.is_file() {
            if !visited_dirs.insert(visit_path.clone()) {
                continue;
            }

            inspect_package(
                config,
                workspace,
                &visit_path,
                &manifest_path,
                jobs,
                blocked,
                blocked_seen,
            )?;

            let nested = visit_path.join("node_modules");
            if nested.is_dir() {
                walk_node_modules(
                    config,
                    workspace,
                    &nested,
                    jobs,
                    blocked,
                    blocked_seen,
                    visited_dirs,
                )?;
            }
        } else {
            walk_node_modules(
                config,
                workspace,
                &visit_path,
                jobs,
                blocked,
                blocked_seen,
                visited_dirs,
            )?;
        }
    }

    Ok(())
}

fn inspect_package(
    config: &SnpmConfig,
    workspace: Option<&Workspace>,
    pkg_root: &Path,
    manifest_path: &Path,
    jobs: &mut Vec<DepScriptJob>,
    blocked: &mut Vec<String>,
    blocked_seen: &mut BTreeSet<String>,
) -> Result<()> {
    let value = read_manifest(manifest_path)?;
    let Some(name) = package_name(&value).filter(|name| !name.is_empty()) else {
        return Ok(());
    };
    let Some(scripts) = package_scripts(&value) else {
        return Ok(());
    };

    if !has_lifecycle_scripts(scripts) {
        return Ok(());
    }

    if !is_dep_script_allowed(config, workspace, name) {
        if blocked_seen.insert(name.to_string()) {
            blocked.push(name.to_string());
        }
        return Ok(());
    }

    let cache_entry = match package_version(&value).filter(|version| !version.is_empty()) {
        Some(version) => Some(SideEffectsCacheEntry::new(config, name, version, pkg_root)?),
        None => None,
    };

    if let Some(entry) = cache_entry.as_ref() {
        match entry.restore_if_available(pkg_root) {
            Ok(SideEffectsCacheRestore::Restored | SideEffectsCacheRestore::AlreadyApplied) => {
                return Ok(());
            }
            Ok(SideEffectsCacheRestore::Miss) => {}
            Err(error) => {
                console::warn(&format!(
                    "failed to restore side-effects cache for {}: {}",
                    name, error
                ));
            }
        }
    }

    jobs.push(DepScriptJob {
        name: name.to_string(),
        pkg_root: pkg_root.to_path_buf(),
        scripts: scripts.clone(),
        cache_entry,
    });
    Ok(())
}

fn run_jobs(jobs: Vec<DepScriptJob>) -> Result<()> {
    if jobs.is_empty() {
        return Ok(());
    }

    let concurrency = lifecycle_script_concurrency();
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(concurrency)
        .build()
        .map_err(|error| SnpmError::Internal {
            reason: format!("could not build lifecycle script worker pool: {error}"),
        })?;

    pool.install(|| jobs.into_par_iter().try_for_each(run_single_job))
}

fn run_single_job(job: DepScriptJob) -> Result<()> {
    let ran = run_present_scripts(&job.name, &job.pkg_root, &job.scripts)?;
    if ran == 0 {
        return Ok(());
    }

    if let Some(entry) = job.cache_entry
        && let Err(error) = entry.save(&job.pkg_root)
    {
        console::warn(&format!(
            "failed to save side-effects cache for {}: {}",
            job.name, error
        ));
    }

    Ok(())
}

fn lifecycle_script_concurrency() -> usize {
    let cpus = std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(2);
    // Most postinstall workloads are a mix of I/O and CPU; oversubscribing
    // wastes context-switches and starves the shell. Cap at four.
    cpus.clamp(1, 4)
}

fn has_lifecycle_scripts(scripts: &serde_json::Map<String, serde_json::Value>) -> bool {
    LIFECYCLE_SCRIPT_NAMES
        .iter()
        .any(|script_name| scripts.contains_key(*script_name))
}

#[cfg(test)]
mod tests {
    use super::run_install_scripts;
    use crate::config::{AuthScheme, HoistingMode, LinkBackend, SnpmConfig};

    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn make_config(data_dir: PathBuf) -> SnpmConfig {
        SnpmConfig {
            cache_dir: data_dir.join("cache"),
            data_dir,
            allow_scripts: BTreeSet::from(["dep".to_string()]),
            disable_global_virtual_store_for_packages: BTreeSet::new(),
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
            frozen_lockfile_default: false,
            always_auth: false,
            registry_concurrency: 64,
            verbose: false,
            log_file: None,
        }
    }

    #[cfg(unix)]
    #[test]
    fn run_install_scripts_restores_cached_side_effects() {
        let dir = tempdir().unwrap();
        let project_root = dir.path();
        let dep_root = project_root.join("node_modules").join("dep");
        let counter = project_root.join("side-effects-counter.txt");
        let built = dep_root.join("built.txt");
        let config = make_config(project_root.join(".snpm-data"));

        fs::create_dir_all(&dep_root).unwrap();
        fs::write(
            dep_root.join("package.json"),
            format!(
                r#"{{
  "name": "dep",
  "version": "1.0.0",
  "scripts": {{
    "postinstall": "echo run >> '{}' && echo built > built.txt"
  }}
}}
"#,
                counter.display()
            ),
        )
        .unwrap();

        run_install_scripts(&config, None, project_root).unwrap();
        assert_eq!(fs::read_to_string(&counter).unwrap().lines().count(), 1);
        assert_eq!(fs::read_to_string(&built).unwrap(), "built\n");

        fs::remove_dir_all(&dep_root).unwrap();
        fs::create_dir_all(&dep_root).unwrap();
        fs::write(
            dep_root.join("package.json"),
            format!(
                r#"{{
  "name": "dep",
  "version": "1.0.0",
  "scripts": {{
    "postinstall": "echo run >> '{}' && echo built > built.txt"
  }}
}}
"#,
                counter.display()
            ),
        )
        .unwrap();

        run_install_scripts(&config, None, project_root).unwrap();

        assert_eq!(fs::read_to_string(&counter).unwrap().lines().count(), 1);
        assert_eq!(fs::read_to_string(&built).unwrap(), "built\n");
    }

    #[cfg(unix)]
    #[test]
    fn dep_lifecycle_scripts_run_in_parallel() {
        use std::time::{Duration, Instant};

        let dir = tempdir().unwrap();
        let project_root = dir.path();
        let mut config = make_config(project_root.join(".snpm-data"));
        config.allow_scripts.insert("dep-a".to_string());
        config.allow_scripts.insert("dep-b".to_string());
        config.allow_scripts.insert("dep-c".to_string());
        config.allow_scripts.insert("dep-d".to_string());

        let dep_names = ["dep-a", "dep-b", "dep-c", "dep-d"];
        for name in dep_names {
            let dep_root = project_root.join("node_modules").join(name);
            fs::create_dir_all(&dep_root).unwrap();
            fs::write(
                dep_root.join("package.json"),
                format!(
                    r#"{{
  "name": "{name}",
  "version": "1.0.0",
  "scripts": {{
    "postinstall": "sleep 0.4"
  }}
}}
"#
                ),
            )
            .unwrap();
        }

        let started = Instant::now();
        run_install_scripts(&config, None, project_root).unwrap();
        let elapsed = started.elapsed();

        // Four 400ms sleeps run sequentially take ~1.6s. With concurrency
        // capped at min(cpus, 4) >= 2 they should comfortably finish in
        // under 1.2s (sequential lower bound 1.6s, parallel ~0.4-0.8s).
        // The threshold is loose to tolerate slow CI without hiding the
        // sequentialization regression.
        assert!(
            elapsed < Duration::from_millis(1300),
            "expected parallel dispatch (<1.3s), got {elapsed:?}; sequential would be >=1.6s"
        );
    }

    #[cfg(unix)]
    #[test]
    fn empty_project_returns_no_blocked_packages() {
        // Project without a node_modules directory at all — collect phase
        // produces an empty job list and the runner short-circuits.
        let dir = tempdir().unwrap();
        let project_root = dir.path();
        let config = make_config(project_root.join(".snpm-data"));

        let blocked = run_install_scripts(&config, None, project_root).unwrap();
        assert!(blocked.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn project_with_only_blocked_packages_runs_nothing() {
        // A package with lifecycle scripts that policy refuses to run must
        // surface in `blocked` and must NOT have its script executed.
        let dir = tempdir().unwrap();
        let project_root = dir.path();
        let counter = project_root.join("counter.txt");
        let config = make_config(project_root.join(".snpm-data"));

        let dep_root = project_root.join("node_modules").join("not-allowed");
        fs::create_dir_all(&dep_root).unwrap();
        fs::write(
            dep_root.join("package.json"),
            format!(
                r#"{{
  "name": "not-allowed",
  "version": "1.0.0",
  "scripts": {{
    "postinstall": "echo SHOULD_NOT_RUN >> '{}'"
  }}
}}
"#,
                counter.display()
            ),
        )
        .unwrap();

        let blocked = run_install_scripts(&config, None, project_root).unwrap();
        assert_eq!(blocked, vec!["not-allowed".to_string()]);
        assert!(
            !counter.exists(),
            "blocked package's script must not execute"
        );
    }

    #[cfg(unix)]
    #[test]
    fn mixed_allowed_and_blocked_dependencies_run_only_allowed() {
        let dir = tempdir().unwrap();
        let project_root = dir.path();
        let allowed_marker = project_root.join("allowed.txt");
        let blocked_marker = project_root.join("blocked.txt");
        let mut config = make_config(project_root.join(".snpm-data"));
        config.allow_scripts.insert("ok-pkg".to_string());

        for (name, marker) in [
            ("ok-pkg", &allowed_marker),
            ("blocked-pkg", &blocked_marker),
        ] {
            let dep_root = project_root.join("node_modules").join(name);
            fs::create_dir_all(&dep_root).unwrap();
            fs::write(
                dep_root.join("package.json"),
                format!(
                    r#"{{
  "name": "{name}",
  "version": "1.0.0",
  "scripts": {{
    "postinstall": "echo ran > '{}'"
  }}
}}
"#,
                    marker.display()
                ),
            )
            .unwrap();
        }

        let blocked = run_install_scripts(&config, None, project_root).unwrap();
        assert_eq!(blocked, vec!["blocked-pkg".to_string()]);
        assert!(allowed_marker.is_file(), "allowed package must have run");
        assert!(
            !blocked_marker.exists(),
            "blocked package must not have run"
        );
    }

    #[cfg(unix)]
    #[test]
    fn one_script_failure_surfaces_an_error() {
        // When a dep's script exits non-zero, the runner must surface the
        // failure (it currently fail-fasts via try_for_each).
        let dir = tempdir().unwrap();
        let project_root = dir.path();
        let mut config = make_config(project_root.join(".snpm-data"));
        config.allow_scripts.insert("good".to_string());
        config.allow_scripts.insert("bad".to_string());

        for (name, body) in [
            ("good", "exit 0"),
            ("bad", "exit 7"),
        ] {
            let dep_root = project_root.join("node_modules").join(name);
            fs::create_dir_all(&dep_root).unwrap();
            fs::write(
                dep_root.join("package.json"),
                format!(
                    r#"{{
  "name": "{name}",
  "version": "1.0.0",
  "scripts": {{
    "postinstall": "{body}"
  }}
}}
"#
                ),
            )
            .unwrap();
        }

        let result = run_install_scripts(&config, None, project_root);
        assert!(
            result.is_err(),
            "a failing dep script must produce an error"
        );
    }

    #[cfg(unix)]
    #[test]
    fn package_without_lifecycle_keys_is_skipped() {
        // A package.json with a `scripts` block that has no lifecycle keys
        // should not consume a job slot or contribute to `blocked`.
        let dir = tempdir().unwrap();
        let project_root = dir.path();
        let config = make_config(project_root.join(".snpm-data"));
        let dep_root = project_root.join("node_modules").join("noop");
        fs::create_dir_all(&dep_root).unwrap();
        fs::write(
            dep_root.join("package.json"),
            r#"{"name":"noop","version":"1.0.0","scripts":{"test":"echo hi"}}"#,
        )
        .unwrap();

        let blocked = run_install_scripts(&config, None, project_root).unwrap();
        assert!(blocked.is_empty());
    }
}
