use super::cold::resolve_cold_install;
use super::plan::ProjectInstallPlan;
use crate::SnpmError;
use crate::console;
use crate::lockfile;
use crate::operations::install::utils::FrozenLockfileMode;
use crate::resolve::{PackageId, ResolutionGraph};
use crate::{Project, Result, SnpmConfig};

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Instant;

use crate::operations::install::utils::{
    InstallOptions, InstallScenario, IntegrityState, ScenarioResult, detect_install_scenario,
    load_graph_snapshot, materialize_missing_packages, validate_graph_min_package_age,
};

pub(super) struct ResolvedInstall {
    pub scenario: InstallScenario,
    pub graph: ResolutionGraph,
    pub store_paths: BTreeMap<PackageId, PathBuf>,
    pub integrity_state: Option<IntegrityState>,
    pub wrote_lockfile: bool,
}

pub(super) async fn resolve_install_state(
    config: &SnpmConfig,
    project: &Project,
    plan: &ProjectInstallPlan,
    options: &InstallOptions,
) -> Result<ResolvedInstall> {
    let planned_root_dependencies = if matches!(options.frozen_lockfile, FrozenLockfileMode::Fix) {
        pinned_root_dependencies_for_fix(plan, config)?
    } else {
        plan.root_dependencies.clone()
    };

    let explicit_lockfile_seed = explicit_lockfile_seed(config, project, plan, options);

    let scenario_result = if options.include_dev
        && plan.additions.is_empty()
        && !matches!(
            options.frozen_lockfile,
            FrozenLockfileMode::No | FrozenLockfileMode::Fix
        ) {
        detect_install_scenario(
            project,
            plan.workspace.as_ref(),
            &plan.lockfile_path,
            &plan.manifest_root,
            &plan.root_specs.optional,
            config,
            options.frozen_lockfile,
            options.force,
            plan.compatible_lockfile.as_ref(),
        )
    } else {
        ScenarioResult::cold()
    };

    let ScenarioResult {
        cache_check,
        graph,
        integrity_state,
        scenario,
        ..
    } = scenario_result;
    let mut store_paths = BTreeMap::new();
    let mut wrote_lockfile = false;

    // Building a reqwest client sets up rustls + a DNS resolver, which is
    // a few milliseconds we don't want on the common Hot no-op that never
    // touches the network. Create it only for scenarios that actually
    // need it: an age check, or any non-Hot (cache/download/resolve) path.
    let needs_network =
        config.min_package_age_days.is_some() || !matches!(scenario, InstallScenario::Hot);
    let registry_client = if needs_network {
        Some(crate::http::create_client()?)
    } else {
        None
    };
    let client = || {
        registry_client
            .as_ref()
            .expect("registry client is initialized for age-checks and non-hot installs")
    };

    // The age check only reaches the network when a minimum age is
    // configured; skip it entirely otherwise so the Hot path never
    // forces the client to exist.
    if config.min_package_age_days.is_some() {
        if let Some(seed_graph) = explicit_lockfile_seed.as_ref() {
            validate_graph_min_package_age(config, client(), seed_graph, options.force).await?;
        }
        if let Some(graph) = graph.as_ref() {
            validate_graph_min_package_age(config, client(), graph, options.force).await?;
        }
    }

    let graph = match scenario {
        InstallScenario::Hot => require_graph(graph, "Hot")?,
        InstallScenario::WarmLinkOnly => {
            let graph = require_graph(graph, "WarmLinkOnly")?;
            let cache_check =
                require_cache(cache_check, "WarmLinkOnly scenario requires cache state")?;

            store_paths = cache_check.cached;
            console::verbose(&format!(
                "warm link-only path: {} packages from cache",
                store_paths.len()
            ));
            console::step_with_count("Using cached packages", store_paths.len());

            graph
        }
        InstallScenario::WarmPartialCache => {
            let graph = require_graph(graph, "WarmPartialCache")?;
            let cache_check = require_cache(
                cache_check,
                "WarmPartialCache scenario requires cache state",
            )?;
            let cached_count = cache_check.cached.len();
            let missing_count = cache_check.missing.len();

            console::verbose(&format!(
                "warm partial-cache path: {} cached, {} to download",
                cached_count, missing_count
            ));

            store_paths = cache_check.cached;

            if !cache_check.missing.is_empty() {
                console::step("Downloading missing packages");
                let materialize_start = Instant::now();
                let downloaded =
                    materialize_missing_packages(config, &cache_check.missing, client()).await?;

                console::verbose(&format!(
                    "downloaded {} missing packages in {:.3}s",
                    downloaded.len(),
                    materialize_start.elapsed().as_secs_f64()
                ));

                store_paths.extend(downloaded);
            }

            console::step_with_count("Resolved, downloaded and extracted", store_paths.len());
            graph
        }
        InstallScenario::Cold => {
            let seed_graph = explicit_lockfile_seed.as_ref().or(graph.as_ref());
            let (graph, resolved_store_paths) = resolve_cold_install(
                config,
                client(),
                plan,
                &planned_root_dependencies,
                options.force,
                seed_graph,
            )
            .await?;

            store_paths = resolved_store_paths;

            if options.include_dev {
                lockfile::write(&plan.lockfile_path, &graph, &plan.root_specs.optional)?;
                wrote_lockfile = true;
            }

            console::step_with_count("Resolved, downloaded and extracted", store_paths.len());
            graph
        }
    };

    if options.include_dev
        && !wrote_lockfile
        && plan.compatible_lockfile.is_some()
        && !plan.lockfile_path.is_file()
    {
        lockfile::write(&plan.lockfile_path, &graph, &plan.root_specs.optional)?;
        wrote_lockfile = true;
    }

    if wrote_lockfile {
        console::step("Saved lockfile");
    }

    Ok(ResolvedInstall {
        scenario,
        graph,
        store_paths,
        integrity_state,
        wrote_lockfile,
    })
}

fn pinned_root_dependencies_for_fix(
    plan: &ProjectInstallPlan,
    config: &SnpmConfig,
) -> Result<BTreeMap<String, String>> {
    let mut root_dependencies = plan.root_dependencies.clone();
    let existing = match read_lockfile_for_fix(plan, config) {
        Ok(lockfile) => lockfile,
        Err(_) => return Ok(root_dependencies),
    };

    for (name, requested) in &plan.root_specs.required {
        let Some(dep) = existing.root.dependencies.get(name) else {
            continue;
        };

        if dep.optional || dep.version.is_none() || dep.requested != *requested {
            continue;
        }

        if let Some(version) = dep.version.as_ref() {
            root_dependencies.insert(name.clone(), version.clone());
        }
    }

    for (name, requested) in &plan.root_specs.optional {
        let Some(dep) = existing.root.dependencies.get(name) else {
            continue;
        };

        if dep.requested != *requested || dep.version.is_none() {
            continue;
        }

        if let Some(version) = dep.version.as_ref() {
            root_dependencies.insert(name.clone(), version.clone());
        }
    }

    Ok(root_dependencies)
}

fn explicit_lockfile_seed(
    config: &SnpmConfig,
    project: &Project,
    plan: &ProjectInstallPlan,
    options: &InstallOptions,
) -> Option<ResolutionGraph> {
    if matches!(options.frozen_lockfile, FrozenLockfileMode::Fix) {
        return read_lockfile_for_fix(plan, config)
            .map(|lockfile| lockfile::to_graph(&lockfile))
            .ok();
    }

    if plan.additions.is_empty() || matches!(options.frozen_lockfile, FrozenLockfileMode::No) {
        return None;
    }

    read_existing_graph_seed(config, project, plan)
}

fn read_existing_graph_seed(
    config: &SnpmConfig,
    project: &Project,
    plan: &ProjectInstallPlan,
) -> Option<ResolutionGraph> {
    if plan.lockfile_path.is_file() {
        if let Some(snapshot) = load_graph_snapshot(&project.root, &plan.lockfile_path) {
            return Some(snapshot.graph);
        }

        return lockfile::read(&plan.lockfile_path)
            .map(|lockfile| lockfile::to_graph(&lockfile))
            .ok();
    }

    let source = plan.compatible_lockfile.as_ref()?;
    lockfile::read_compatible_lockfile(source, config)
        .map(|lockfile| lockfile::to_graph(&lockfile))
        .ok()
}

fn read_lockfile_for_fix(
    plan: &ProjectInstallPlan,
    config: &SnpmConfig,
) -> Result<crate::lockfile::Lockfile> {
    if plan.lockfile_path.is_file() {
        return lockfile::read(&plan.lockfile_path);
    }

    let source = plan
        .compatible_lockfile
        .as_ref()
        .ok_or_else(|| crate::SnpmError::Lockfile {
            path: plan.lockfile_path.clone(),
            reason: "no lockfile was found".into(),
        })?;

    lockfile::read_compatible_lockfile(source, config)
}

fn require_graph(graph: Option<ResolutionGraph>, scenario: &str) -> Result<ResolutionGraph> {
    graph.ok_or_else(|| SnpmError::Internal {
        reason: format!("{scenario} scenario requires a precomputed graph"),
    })
}

fn require_cache<T>(cache_check: Option<T>, message: &str) -> Result<T> {
    cache_check.ok_or_else(|| SnpmError::Internal {
        reason: message.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        ProjectInstallPlan, explicit_lockfile_seed, pinned_root_dependencies_for_fix,
        read_lockfile_for_fix,
    };
    use crate::Project;
    use crate::config::SnpmConfig;
    use crate::lockfile::{LockRoot, LockRootDependency, Lockfile};
    use crate::operations::install::manifest::RootSpecSet;
    use crate::operations::install::utils::{FrozenLockfileMode, InstallOptions};
    use crate::project::Manifest;
    use serde_yaml;
    use std::collections::{BTreeMap, BTreeSet};

    use tempfile::tempdir;

    type LockfileDep<'a> = (&'a str, (&'a str, Option<&'a str>, bool));

    fn make_config() -> SnpmConfig {
        SnpmConfig::for_tests()
    }

    fn make_project(root: std::path::PathBuf) -> Project {
        Project {
            manifest_path: root.join("package.json"),
            root,
            manifest: Manifest {
                name: Some("app".to_string()),
                version: Some("1.0.0".to_string()),
                ..Manifest::default()
            },
        }
    }

    fn make_install_options(
        requested: Vec<String>,
        frozen_lockfile: FrozenLockfileMode,
    ) -> InstallOptions {
        InstallOptions {
            requested,
            dev: false,
            include_dev: true,
            frozen_lockfile,
            strict_no_lockfile: false,
            force: false,
            silent_summary: false,
        }
    }

    fn make_plan(
        lockfile_path: std::path::PathBuf,
        required: BTreeMap<String, String>,
        optional: BTreeMap<String, String>,
    ) -> ProjectInstallPlan {
        let mut root_dependencies = required.clone();
        root_dependencies.extend(optional.clone());

        ProjectInstallPlan {
            workspace: None,
            catalog: None,
            overrides: BTreeMap::new(),
            additions: BTreeMap::new(),
            local_deps: BTreeSet::new(),
            local_dev_deps: BTreeSet::new(),
            local_optional_deps: BTreeSet::new(),
            manifest_root: BTreeMap::new(),
            root_specs: RootSpecSet { required, optional },
            root_dependencies,
            root_protocols: BTreeMap::new(),
            optional_root_names: BTreeSet::new(),
            lockfile_path,
            compatible_lockfile: None,
            is_fresh_install: true,
        }
    }

    fn write_lockfile(path: &std::path::Path, deps: Vec<LockfileDep<'_>>) {
        let lockfile = Lockfile {
            version: 1,
            root: LockRoot {
                dependencies: deps
                    .into_iter()
                    .map(|(name, (requested, version, optional))| {
                        (
                            name.to_string(),
                            LockRootDependency {
                                requested: requested.to_string(),
                                package: None,
                                version: version.map(ToString::to_string),
                                optional,
                            },
                        )
                    })
                    .collect(),
            },
            packages: BTreeMap::new(),
        };

        let lockfile_path = path.join("snpm-lock.yaml");
        let data = serde_yaml::to_string(&lockfile).unwrap();
        std::fs::write(&lockfile_path, data).unwrap();
    }

    #[test]
    fn pinned_root_dependencies_for_fix_rewrites_matching_required_and_optional() {
        let dir = tempdir().unwrap();
        write_lockfile(
            dir.path(),
            vec![
                ("left", ("^1.0.0", Some("1.2.3"), false)),
                ("opt", ("^2.0.0", Some("2.0.1"), true)),
            ],
        );
        let config = make_config();
        let plan = make_plan(
            dir.path().join("snpm-lock.yaml"),
            BTreeMap::from([("left".to_string(), "^1.0.0".to_string())]),
            BTreeMap::from([("opt".to_string(), "^2.0.0".to_string())]),
        );

        let pinned = pinned_root_dependencies_for_fix(&plan, &config).unwrap();
        let expected = BTreeMap::from([
            ("left".to_string(), "1.2.3".to_string()),
            ("opt".to_string(), "2.0.1".to_string()),
        ]);
        assert_eq!(pinned, expected);
        assert_eq!(plan.lockfile_path, dir.path().join("snpm-lock.yaml"));
        assert_eq!(
            pinned_root_dependencies_for_fix(&plan, &config).unwrap(),
            expected
        );
    }

    #[test]
    fn pinned_root_dependencies_for_fix_keeps_ranges_when_request_changed() {
        let dir = tempdir().unwrap();
        write_lockfile(dir.path(), vec![("left", ("^1.0.0", Some("1.2.3"), false))]);
        let config = make_config();
        let plan = make_plan(
            dir.path().join("snpm-lock.yaml"),
            BTreeMap::from([("left".to_string(), "^2.0.0".to_string())]),
            BTreeMap::new(),
        );

        let pinned = pinned_root_dependencies_for_fix(&plan, &config).unwrap();
        let expected = BTreeMap::from([("left".to_string(), "^2.0.0".to_string())]);
        assert_eq!(pinned, expected);
    }

    #[test]
    fn pinned_root_dependencies_for_fix_uses_unlocked_dependency_without_lockfile() {
        let dir = tempdir().unwrap();
        let config = make_config();
        let plan = make_plan(
            dir.path().join("missing-lockfile.yaml"),
            BTreeMap::from([("left".to_string(), "^1.0.0".to_string())]),
            BTreeMap::new(),
        );

        let pinned = pinned_root_dependencies_for_fix(&plan, &config).unwrap();
        let expected = BTreeMap::from([("left".to_string(), "^1.0.0".to_string())]);
        assert_eq!(pinned, expected);
    }

    #[test]
    fn read_lockfile_for_fix_reads_workspace_lockfile_when_available() {
        let dir = tempdir().unwrap();
        write_lockfile(dir.path(), vec![("left", ("^1.0.0", Some("1.2.3"), false))]);
        let config = make_config();
        let plan = make_plan(
            dir.path().join("snpm-lock.yaml"),
            BTreeMap::from([("left".to_string(), "^1.0.0".to_string())]),
            BTreeMap::new(),
        );

        let loaded = read_lockfile_for_fix(&plan, &config).unwrap();
        assert_eq!(
            loaded.root.dependencies["left"].version.as_deref(),
            Some("1.2.3")
        );
        assert_eq!(
            loaded.root.dependencies["left"].requested,
            "^1.0.0".to_string()
        );
    }

    #[test]
    fn explicit_lockfile_seed_reuses_existing_graph_for_additions() {
        let dir = tempdir().unwrap();
        write_lockfile(dir.path(), vec![("left", ("^1.0.0", Some("1.2.3"), false))]);
        let config = make_config();
        let project = make_project(dir.path().to_path_buf());
        let mut plan = make_plan(
            dir.path().join("snpm-lock.yaml"),
            BTreeMap::from([("left".to_string(), "^1.0.0".to_string())]),
            BTreeMap::new(),
        );
        plan.additions
            .insert("right".to_string(), "^2.0.0".to_string());
        plan.root_dependencies
            .insert("right".to_string(), "^2.0.0".to_string());
        let options =
            make_install_options(vec!["right@^2.0.0".to_string()], FrozenLockfileMode::Prefer);

        let graph = explicit_lockfile_seed(&config, &project, &plan, &options).unwrap();

        assert_eq!(
            graph
                .root
                .dependencies
                .get("left")
                .map(|dep| dep.resolved.version.as_str()),
            Some("1.2.3")
        );
        assert!(!graph.root.dependencies.contains_key("right"));
    }

    #[test]
    fn explicit_lockfile_seed_respects_disabled_lockfile_mode() {
        let dir = tempdir().unwrap();
        write_lockfile(dir.path(), vec![("left", ("^1.0.0", Some("1.2.3"), false))]);
        let config = make_config();
        let project = make_project(dir.path().to_path_buf());
        let mut plan = make_plan(
            dir.path().join("snpm-lock.yaml"),
            BTreeMap::from([("left".to_string(), "^1.0.0".to_string())]),
            BTreeMap::new(),
        );
        plan.additions
            .insert("right".to_string(), "^2.0.0".to_string());
        let options =
            make_install_options(vec!["right@^2.0.0".to_string()], FrozenLockfileMode::No);

        assert!(explicit_lockfile_seed(&config, &project, &plan, &options).is_none());
    }

    #[test]
    fn pinned_root_dependencies_for_fix_does_not_use_incomplete_optional_match() {
        let dir = tempdir().unwrap();
        write_lockfile(dir.path(), vec![("opt", ("^2.0.0", None, true))]);
        let config = make_config();
        let plan = make_plan(
            dir.path().join("snpm-lock.yaml"),
            BTreeMap::new(),
            BTreeMap::from([("opt".to_string(), "^2.0.0".to_string())]),
        );

        let pinned = pinned_root_dependencies_for_fix(&plan, &config).unwrap();
        let expected = BTreeMap::from([("opt".to_string(), "^2.0.0".to_string())]);
        assert_eq!(pinned, expected);
    }

    #[test]
    fn read_lockfile_for_fix_errors_without_lockfile_or_compatible_source() {
        let config = make_config();
        let missing_path = tempdir().unwrap().path().join("missing.yaml");
        let plan = make_plan(missing_path, BTreeMap::new(), BTreeMap::new());

        assert!(read_lockfile_for_fix(&plan, &config).is_err());
    }
}
