use super::super::engine::{ResolverContext, ResolverState};
use super::super::resolve_with_optional_roots_with_seed;
use super::super::types::{PackageId, ResolutionGraph, ResolutionRoot, RootDependency, ResolvedPackage};
use crate::config::{OfflineMode, SnpmConfig};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

fn make_config() -> SnpmConfig {
    SnpmConfig {
        cache_dir: std::path::Path::new("/tmp/cache").to_path_buf(),
        data_dir: std::path::Path::new("/tmp/data").to_path_buf(),
        allow_scripts: BTreeSet::new(),
        min_package_age_days: None,
        min_package_cache_age_days: None,
        default_registry: "https://registry.npmjs.org".to_string(),
        scoped_registries: BTreeMap::new(),
        registry_auth: BTreeMap::new(),
        default_registry_auth_token: None,
        default_registry_auth_scheme: crate::config::AuthScheme::Bearer,
        registry_auth_schemes: BTreeMap::new(),
        hoisting: crate::config::HoistingMode::SingleVersion,
        link_backend: crate::config::LinkBackend::Auto,
        strict_peers: false,
        frozen_lockfile_default: false,
        always_auth: false,
        registry_concurrency: 64,
        verbose: false,
        log_file: None,
    }
}

fn make_seed_graph_for_chain() -> ResolutionGraph {
    let foo = PackageId {
        name: "foo".to_string(),
        version: "1.2.3".to_string(),
    };
    let bar = PackageId {
        name: "bar".to_string(),
        version: "2.3.4".to_string(),
    };

    ResolutionGraph {
        root: ResolutionRoot {
            dependencies: BTreeMap::from([(
                "foo".to_string(),
                RootDependency {
                    requested: "^1.0.0".to_string(),
                    resolved: foo.clone(),
                },
            )]),
        },
        packages: BTreeMap::from([
            (
                foo.clone(),
                ResolvedPackage {
                    id: foo.clone(),
                    tarball: "https://registry.example/foo.tgz".to_string(),
                    integrity: None,
                    dependencies: BTreeMap::from([("bar".to_string(), bar.clone())]),
                    peer_dependencies: BTreeMap::new(),
                    bundled_dependencies: None,
                    has_bin: false,
                },
            ),
            (
                bar.clone(),
                ResolvedPackage {
                    id: bar.clone(),
                    tarball: "https://registry.example/bar.tgz".to_string(),
                    integrity: None,
                    dependencies: BTreeMap::new(),
                    peer_dependencies: BTreeMap::new(),
                    bundled_dependencies: None,
                    has_bin: false,
                },
            ),
        ]),
    }
}

fn make_parent_scoped_graph() -> (ResolutionGraph, PackageId, PackageId, PackageId) {
    let foo = PackageId {
        name: "foo".to_string(),
        version: "1.0.0".to_string(),
    };
    let bar_parent = PackageId {
        name: "bar".to_string(),
        version: "1.0.1".to_string(),
    };
    let bar_root = PackageId {
        name: "bar".to_string(),
        version: "2.0.0".to_string(),
    };

    (
        ResolutionGraph {
            root: ResolutionRoot {
                dependencies: BTreeMap::from([
                    (
                        "foo".to_string(),
                        RootDependency {
                            requested: "^1.0.0".to_string(),
                            resolved: foo.clone(),
                        },
                    ),
                    (
                        "bar".to_string(),
                        RootDependency {
                            requested: "^2.0.0".to_string(),
                            resolved: bar_root.clone(),
                        },
                    ),
                ]),
            },
            packages: BTreeMap::from([
                (
                    foo.clone(),
                    ResolvedPackage {
                        id: foo.clone(),
                        tarball: "https://registry.example/foo.tgz".to_string(),
                        integrity: None,
                        dependencies: BTreeMap::from([("bar".to_string(), bar_parent.clone())]),
                        peer_dependencies: BTreeMap::new(),
                        bundled_dependencies: None,
                        has_bin: false,
                    },
                ),
                (
                    bar_parent.clone(),
                    ResolvedPackage {
                        id: bar_parent.clone(),
                        tarball: "https://registry.example/bar-parent.tgz".to_string(),
                        integrity: None,
                        dependencies: BTreeMap::new(),
                        peer_dependencies: BTreeMap::new(),
                        bundled_dependencies: None,
                        has_bin: false,
                    },
                ),
                (
                    bar_root.clone(),
                    ResolvedPackage {
                        id: bar_root.clone(),
                        tarball: "https://registry.example/bar-root.tgz".to_string(),
                        integrity: None,
                        dependencies: BTreeMap::new(),
                        peer_dependencies: BTreeMap::new(),
                        bundled_dependencies: None,
                        has_bin: false,
                    },
                ),
            ]),
        },
        foo,
        bar_parent,
        bar_root,
    )
}

fn make_context<'a>(
    config: &'a SnpmConfig,
    client: &'a reqwest::Client,
    existing_graph: &'a ResolutionGraph,
) -> ResolverContext<'a> {
    let state = ResolverState::new(64);
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();

    ResolverContext {
        config,
        client,
        min_age_days: None,
        force: false,
        overrides: None,
        existing_graph: Some(existing_graph),
        offline_mode: OfflineMode::Online,
        state,
        prefetch_tx: tx,
    }
}

#[tokio::test]
async fn resolve_with_seeded_graph_reuses_complete_seed_graph() {
    let config = make_config();
    let client = reqwest::Client::new();
    let seed_graph = make_seed_graph_for_chain();
    let call_count = Arc::new(AtomicUsize::new(0));

    let graph = resolve_with_optional_roots_with_seed(
        &config,
        &client,
        &BTreeMap::from([("foo".to_string(), "^1.0.0".to_string())]),
        &BTreeMap::new(),
        &BTreeSet::new(),
        None,
        false,
        None,
        Some(&seed_graph),
        {
            let call_count = call_count.clone();
            move |_package| {
                call_count.fetch_add(1, Ordering::SeqCst);
                async move { Ok::<(), crate::SnpmError>(()) }
            }
        },
    )
    .await
    .unwrap();

    let foo_id = PackageId {
        name: "foo".to_string(),
        version: "1.2.3".to_string(),
    };
    let bar_id = PackageId {
        name: "bar".to_string(),
        version: "2.3.4".to_string(),
    };

    assert_eq!(graph.root.dependencies["foo"].resolved, foo_id);
    assert_eq!(graph.packages[&foo_id].dependencies["bar"], bar_id);
    assert_eq!(graph.packages.len(), 2);
    assert_eq!(call_count.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn resolve_with_seeded_graph_prefers_parent_scoped_dependency_over_root_candidate() {
    let config = make_config();
    let client = reqwest::Client::new();
    let (seed_graph, foo_id, bar_parent_id, bar_root_id) = make_parent_scoped_graph();

    let graph = resolve_with_optional_roots_with_seed(
        &config,
        &client,
        &BTreeMap::from([
            ("foo".to_string(), "^1.0.0".to_string()),
            ("bar".to_string(), "^2.0.0".to_string()),
        ]),
        &BTreeMap::new(),
        &BTreeSet::new(),
        None,
        false,
        None,
        Some(&seed_graph),
        |_package| async move { Ok::<(), crate::SnpmError>(()) },
    )
    .await
    .unwrap();

    assert_eq!(graph.root.dependencies["foo"].resolved, foo_id);
    assert_eq!(graph.root.dependencies["bar"].resolved, bar_root_id);
    assert_eq!(graph.packages[&foo_id].dependencies["bar"], bar_parent_id);
}

#[test]
fn seeded_dependency_id_filters_incompatible_range_and_uses_parent_lookup() {
    let config = make_config();
    let client = reqwest::Client::new();
    let seed_graph = make_parent_scoped_graph().0;
    let context = make_context(&config, &client, &seed_graph);

    let foo_id = PackageId {
        name: "foo".to_string(),
        version: "1.0.0".to_string(),
    };
    let bar_parent_id = PackageId {
        name: "bar".to_string(),
        version: "1.0.1".to_string(),
    };

    assert_eq!(
        context.seeded_dependency_id("foo", "^1.0.0", None, &seed_graph),
        Some(foo_id.clone())
    );
    assert!(context.seeded_dependency_id("foo", "^2.0.0", None, &seed_graph).is_none());
    assert_eq!(
        context.seeded_dependency_id("bar", "^1.0.0", Some(&foo_id), &seed_graph),
        Some(bar_parent_id)
    );
}

#[tokio::test]
async fn import_seed_package_chain_imports_complete_graph_and_reports_missing() {
    let config = make_config();
    let client = reqwest::Client::new();
    let complete_graph = make_seed_graph_for_chain();

    let context = make_context(&config, &client, &complete_graph);
    let foo_id = PackageId {
        name: "foo".to_string(),
        version: "1.2.3".to_string(),
    };

    assert!(context.seeded_subgraph_complete(foo_id.clone(), &complete_graph));
    assert!(context
        .import_seed_package_chain(foo_id.clone(), &complete_graph)
        .await
        .unwrap());
    assert!(context.package_already_resolved(&foo_id).await);

    let partial_graph = ResolutionGraph {
        root: complete_graph.root.clone(),
        packages: BTreeMap::from([(
            foo_id.clone(),
            complete_graph.packages[&foo_id].clone(),
        )]),
    };
    let missing_context = make_context(&config, &client, &partial_graph);

    assert!(!missing_context.seeded_subgraph_complete(foo_id.clone(), &partial_graph));
    assert!(!missing_context
        .import_seed_package_chain(foo_id, &partial_graph)
        .await
        .unwrap());
}
