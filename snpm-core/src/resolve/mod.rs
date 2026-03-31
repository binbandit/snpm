mod engine;
mod package;
pub mod peers;
pub mod query;
mod source;
pub mod types;

use crate::config::OfflineMode;
use crate::registry::RegistryProtocol;
use crate::{Result, SnpmConfig};
use reqwest::Client;
use std::collections::{BTreeMap, BTreeSet};

pub use engine::resolve_with_offline;
pub use peers::validate_peers;
pub use types::*;

/// Resolve dependencies with default online mode.
#[allow(clippy::too_many_arguments)]
pub async fn resolve<F, Fut>(
    config: &SnpmConfig,
    client: &Client,
    root_deps: &BTreeMap<String, String>,
    root_protocols: &BTreeMap<String, RegistryProtocol>,
    min_age_days: Option<u32>,
    force: bool,
    overrides: Option<&BTreeMap<String, String>>,
    on_package: F,
) -> Result<ResolutionGraph>
where
    F: FnMut(ResolvedPackage) -> Fut + Send,
    Fut: std::future::Future<Output = Result<()>> + Send,
{
    resolve_with_optional_roots(
        config,
        client,
        root_deps,
        root_protocols,
        &BTreeSet::new(),
        min_age_days,
        force,
        overrides,
        on_package,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn resolve_with_optional_roots<F, Fut>(
    config: &SnpmConfig,
    client: &Client,
    root_deps: &BTreeMap<String, String>,
    root_protocols: &BTreeMap<String, RegistryProtocol>,
    optional_root_names: &BTreeSet<String>,
    min_age_days: Option<u32>,
    force: bool,
    overrides: Option<&BTreeMap<String, String>>,
    on_package: F,
) -> Result<ResolutionGraph>
where
    F: FnMut(ResolvedPackage) -> Fut + Send,
    Fut: std::future::Future<Output = Result<()>> + Send,
{
    engine::resolve_with_offline(
        config,
        client,
        root_deps,
        root_protocols,
        optional_root_names,
        min_age_days,
        force,
        overrides,
        OfflineMode::Online,
        on_package,
    )
    .await
}
