mod config;
mod manifest;
mod results;

use crate::console;
use crate::http;
use crate::registry::RegistryProtocol;
use crate::resolve;
use crate::{Project, Result, SnpmConfig, SnpmError, Workspace};

use reqwest::Client;
use std::collections::BTreeMap;
use std::time::Instant;

use config::{load_catalog, load_overrides};
use manifest::{build_root_dependencies, build_root_protocols, resolve_manifest_dependencies};
use results::{build_outdated_entries, read_current_versions};

use super::super::utils::OutdatedEntry;

pub async fn outdated(
    config: &SnpmConfig,
    project: &Project,
    include_dev: bool,
    force: bool,
) -> Result<Vec<OutdatedEntry>> {
    let workspace = Workspace::discover(&project.root)?;
    let registry_client = http::create_client()?;
    let overrides = load_overrides(project, workspace.as_ref())?;
    let catalog = load_catalog(project, workspace.as_ref())?;
    let resolved_manifest =
        resolve_manifest_dependencies(project, workspace.as_ref(), catalog.as_ref())?;

    let root_dependencies = build_root_dependencies(
        project,
        workspace.as_ref(),
        &resolved_manifest.dependencies,
        &resolved_manifest.development_dependencies,
        include_dev,
    )?;
    let root_protocols = build_root_protocols(&root_dependencies, &resolved_manifest.protocols);

    console::verbose(&format!(
        "outdated: resolving {} root deps (include_dev={} force={})",
        root_dependencies.len(),
        include_dev,
        force
    ));

    let resolve_started = Instant::now();
    let graph = resolve::resolve(
        config,
        &registry_client,
        &root_dependencies,
        &root_protocols,
        config.min_package_age_days,
        force,
        Some(&overrides),
        None,
        |_package| async { Ok::<(), SnpmError>(()) },
    )
    .await?;

    console::verbose(&format!(
        "outdated: resolve completed in {:.3}s (packages={})",
        resolve_started.elapsed().as_secs_f64(),
        graph.packages.len()
    ));

    let current_versions = read_current_versions(project, workspace.as_ref())?;
    let mut entries = build_outdated_entries(
        include_dev,
        &resolved_manifest.dependencies,
        &resolved_manifest.development_dependencies,
        &current_versions,
        &graph.root.dependencies,
    );

    let failed_lookups = enrich_with_latest(
        config,
        &registry_client,
        &root_protocols,
        &resolved_manifest.dependencies,
        &resolved_manifest.development_dependencies,
        &mut entries,
    )
    .await;

    // A failed lookup leaves `latest` as None, which the retain below
    // treats like "nothing newer" — so a registry blip would silently
    // hide exactly the beyond-range upgrades this report exists to show.
    // Say so out loud instead of only in verbose logs.
    if failed_lookups > 0 {
        console::warn(&format!(
            "could not determine the latest version for {failed_lookups} dependenc{} — \
             the report may be missing upgrades beyond your ranges",
            if failed_lookups == 1 { "y" } else { "ies" }
        ));
    }

    // A dep whose installed version already satisfies the range is only
    // worth reporting when the registry has something newer than the
    // range allows — that is the whole point of the `latest` column. An
    // exactly-pinned dep would otherwise never surface its newer major:
    // the pre-latest report skipped it before the enrichment could run.
    entries.retain(|entry| {
        let satisfied = entry.current.as_deref() == Some(entry.wanted.as_str());
        if !satisfied {
            return true;
        }
        entry
            .latest
            .as_deref()
            .is_some_and(|latest| version_is_newer(latest, &entry.wanted))
    });

    Ok(entries)
}

fn version_is_newer(candidate: &str, baseline: &str) -> bool {
    match (
        snpm_semver::parse_version(candidate),
        snpm_semver::parse_version(baseline),
    ) {
        (Ok(candidate), Ok(baseline)) => candidate > baseline,
        _ => false,
    }
}

/// Fill in each entry's `latest` (the registry `latest` dist-tag) so the
/// report can show upgrades beyond the manifest range. resolve already
/// warmed the metadata cache, so these lookups are typically cache hits;
/// failures and non-registry protocols leave `latest` as `None`. Returns
/// how many lookups failed so the caller can flag a possibly-incomplete
/// report.
async fn enrich_with_latest(
    config: &SnpmConfig,
    client: &Client,
    root_protocols: &BTreeMap<String, RegistryProtocol>,
    dependencies: &BTreeMap<String, String>,
    development_dependencies: &BTreeMap<String, String>,
    entries: &mut [OutdatedEntry],
) -> usize {
    let npm = RegistryProtocol::npm();
    let mut failed = 0;
    for entry in entries.iter_mut() {
        let protocol = root_protocols.get(&entry.name).unwrap_or(&npm);

        // Only plain npm-registry deps carry a fetchable "latest" here.
        // jsr deps always come from a `jsr:` spec, which the
        // special-protocol check below skips (the edge name is not the
        // registry package name), so listing jsr in this allowlist would
        // be dead code — supporting it needs alias-aware fetching.
        if protocol.name != "npm" {
            continue;
        }

        // An aliased spec (`"foo": "npm:bar@^1"`) or other special
        // protocol means `entry.name` is NOT the registry package name —
        // fetching by it would report an unrelated package's latest.
        let spec = dependencies
            .get(&entry.name)
            .or_else(|| development_dependencies.get(&entry.name));
        if spec.is_some_and(|spec| {
            crate::operations::install::manifest::is_special_protocol_spec(spec)
                || spec.starts_with("file:")
        }) {
            continue;
        }

        match crate::registry::fetch_package(config, client, &entry.name, protocol).await {
            Ok(package) => entry.latest = package.dist_tags.get("latest").cloned(),
            Err(error) => {
                failed += 1;
                console::verbose(&format!(
                    "outdated: failed to fetch latest for {}: {error}",
                    entry.name
                ));
            }
        }
    }

    failed
}
