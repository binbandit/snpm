use super::super::policy::is_dep_script_allowed;
use super::cache::{SideEffectsCacheEntry, SideEffectsCacheRestore};
use super::execute::{DEPENDENCY_LIFECYCLE_SCRIPT_NAMES, run_present_scripts};
use super::manifest::{
    package_bin, package_name, package_runtime_dep_names, package_scripts, package_version,
    read_manifest,
};
use crate::console;
use crate::project::BinField;
use crate::{Result, SnpmConfig, SnpmError, Workspace};

use rayon::prelude::*;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

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
    bin: Option<BinField>,
    cache_entry: Option<SideEffectsCacheEntry>,
    /// Names listed under this package's `dependencies` / `optional` /
    /// `peer` blocks. Used by the topological sequencer to order
    /// scripts when a producer/consumer relationship exists between
    /// two packages whose postinstalls both run.
    dep_names: Vec<String>,
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
        bin: package_bin(&value),
        cache_entry,
        dep_names: package_runtime_dep_names(&value),
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

    let chunks = topological_chunks(jobs);
    for chunk in chunks {
        pool.install(|| chunk.into_par_iter().try_for_each(run_single_job))?;
    }
    Ok(())
}

/// Partition `jobs` into dep-ordered chunks: chunk N depends only on
/// jobs in chunks 0..N-1, so members within a chunk are safe to run
/// in parallel and chunks run sequentially. A cycle in the dep
/// subgraph collapses to a final warn-and-run chunk — the upstream
/// behavior pacquet inherits from pnpm's `runGroups`.
fn topological_chunks(jobs: Vec<DepScriptJob>) -> Vec<Vec<DepScriptJob>> {
    let mut by_name: HashMap<String, DepScriptJob> = jobs
        .into_iter()
        .map(|job| (job.name.clone(), job))
        .collect();
    let job_names: HashSet<String> = by_name.keys().cloned().collect();

    let mut in_degree: HashMap<String, usize> = HashMap::with_capacity(by_name.len());
    let mut dependents: HashMap<String, Vec<String>> = HashMap::new();
    for (name, job) in &by_name {
        let mut deg = 0;
        for dep in &job.dep_names {
            if dep != name && job_names.contains(dep) {
                deg += 1;
                dependents
                    .entry(dep.clone())
                    .or_default()
                    .push(name.clone());
            }
        }
        in_degree.insert(name.clone(), deg);
    }

    let mut chunks: Vec<Vec<DepScriptJob>> = Vec::new();
    let mut ready: Vec<String> = in_degree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(name, _)| name.clone())
        .collect();

    while !ready.is_empty() {
        let mut chunk = Vec::with_capacity(ready.len());
        let mut next_ready = Vec::new();
        for name in ready {
            if let Some(job) = by_name.remove(&name) {
                chunk.push(job);
            }
            if let Some(consumers) = dependents.remove(&name) {
                for consumer in consumers {
                    if let Some(degree) = in_degree.get_mut(&consumer) {
                        *degree = degree.saturating_sub(1);
                        if *degree == 0 {
                            next_ready.push(consumer);
                        }
                    }
                }
            }
        }
        if !chunk.is_empty() {
            chunks.push(chunk);
        }
        ready = next_ready;
    }

    if !by_name.is_empty() {
        let mut cycle_names: Vec<String> = by_name.keys().cloned().collect();
        cycle_names.sort();
        console::warn(&format!(
            "dependency cycle among lifecycle scripts: {} — running them in parallel",
            cycle_names.join(", ")
        ));
        chunks.push(by_name.into_values().collect());
    }

    chunks
}

fn run_single_job(job: DepScriptJob) -> Result<()> {
    let ran = run_present_scripts(
        &job.name,
        None,
        &job.pkg_root,
        &job.scripts,
        job.bin.as_ref(),
        &DEPENDENCY_LIFECYCLE_SCRIPT_NAMES,
    )?;
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
    DEPENDENCY_LIFECYCLE_SCRIPT_NAMES
        .iter()
        .any(|script_name| scripts.contains_key(*script_name))
}

#[cfg(test)]
mod tests {
    use super::{DepScriptJob, run_install_scripts, topological_chunks};
    use crate::config::SnpmConfig;

    use std::collections::BTreeSet;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn make_job(name: &str, deps: &[&str]) -> DepScriptJob {
        DepScriptJob {
            name: name.to_string(),
            pkg_root: PathBuf::from("/dev/null"),
            scripts: serde_json::Map::new(),
            bin: None,
            cache_entry: None,
            dep_names: deps.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn chunk_names(chunks: &[Vec<DepScriptJob>]) -> Vec<Vec<String>> {
        chunks
            .iter()
            .map(|chunk| {
                let mut names: Vec<String> = chunk.iter().map(|j| j.name.clone()).collect();
                names.sort();
                names
            })
            .collect()
    }

    #[test]
    fn topological_chunks_orders_producer_before_consumer() {
        // builder produces an artifact; consumer reads it.
        let chunks = topological_chunks(vec![
            make_job("consumer", &["builder"]),
            make_job("builder", &[]),
        ]);
        let names = chunk_names(&chunks);
        assert_eq!(names, vec![vec!["builder"], vec!["consumer"]]);
    }

    #[test]
    fn topological_chunks_runs_independents_in_same_chunk() {
        // No deps between any of them — all in the first chunk.
        let chunks = topological_chunks(vec![
            make_job("a", &[]),
            make_job("b", &[]),
            make_job("c", &[]),
        ]);
        let names = chunk_names(&chunks);
        assert_eq!(names, vec![vec!["a", "b", "c"]]);
    }

    #[test]
    fn topological_chunks_ignores_deps_outside_the_job_set() {
        // pkg depends on `react`, but `react` has no lifecycle scripts and
        // therefore no job. The sequencer must treat pkg as having no
        // in-degree constraints, not block on a non-existent producer.
        let chunks = topological_chunks(vec![make_job("pkg", &["react"])]);
        let names = chunk_names(&chunks);
        assert_eq!(names, vec![vec!["pkg"]]);
    }

    #[test]
    fn topological_chunks_collapses_cycles_into_one_chunk() {
        // a -> b -> a forms a cycle. The sequencer warns and runs them
        // together rather than aborting; this mirrors pnpm's behavior.
        let chunks = topological_chunks(vec![make_job("a", &["b"]), make_job("b", &["a"])]);
        let names = chunk_names(&chunks);
        assert_eq!(names, vec![vec!["a", "b"]]);
    }

    #[test]
    fn topological_chunks_three_level_chain() {
        // c depends on b depends on a — three separate chunks of one.
        let chunks = topological_chunks(vec![
            make_job("a", &[]),
            make_job("b", &["a"]),
            make_job("c", &["b"]),
        ]);
        let names = chunk_names(&chunks);
        assert_eq!(names, vec![vec!["a"], vec!["b"], vec!["c"]]);
    }

    #[test]
    fn topological_chunks_diamond_keeps_root_alone_then_fans_out_then_joins() {
        // root <- left, right; sink <- left, right.
        let chunks = topological_chunks(vec![
            make_job("root", &[]),
            make_job("left", &["root"]),
            make_job("right", &["root"]),
            make_job("sink", &["left", "right"]),
        ]);
        let names = chunk_names(&chunks);
        assert_eq!(
            names,
            vec![vec!["root"], vec!["left", "right"], vec!["sink"]]
        );
    }

    fn make_config(data_dir: PathBuf) -> SnpmConfig {
        SnpmConfig {
            cache_dir: data_dir.join("cache"),
            data_dir,
            allow_scripts: BTreeSet::from(["dep".to_string()]),
            ..SnpmConfig::for_tests()
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

        for (name, body) in [("good", "exit 0"), ("bad", "exit 7")] {
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

    #[cfg(unix)]
    #[test]
    fn dependency_prepare_script_does_not_run() {
        // Registry-installed deps must NOT have `prepare` run. Real packages
        // (globals, husky, rollup, etc.) ship `prepare` scripts that call
        // `tsc`/`rollup`/etc. — tools that aren't in the published tarball.
        // npm and pnpm only run `prepare` for git/local deps, never registry.
        let dir = tempdir().unwrap();
        let project_root = dir.path();
        let prepare_marker = project_root.join("prepare-ran.txt");
        let postinstall_marker = project_root.join("postinstall-ran.txt");
        let mut config = make_config(project_root.join(".snpm-data"));
        config.allow_scripts.insert("dep".to_string());

        let dep_root = project_root.join("node_modules").join("dep");
        fs::create_dir_all(&dep_root).unwrap();
        fs::write(
            dep_root.join("package.json"),
            format!(
                r#"{{
  "name": "dep",
  "version": "1.0.0",
  "scripts": {{
    "postinstall": "echo ran > '{}'",
    "prepare": "echo ran > '{}'"
  }}
}}
"#,
                postinstall_marker.display(),
                prepare_marker.display()
            ),
        )
        .unwrap();

        run_install_scripts(&config, None, project_root).unwrap();
        assert!(
            postinstall_marker.is_file(),
            "postinstall must still run for deps"
        );
        assert!(
            !prepare_marker.exists(),
            "prepare must NOT run for registry-installed deps"
        );
    }
}
