mod packages;
mod registry;
mod root;
mod state;

use super::super::types::ResolvedPackage;
use crate::SnpmConfig;
use crate::config::OfflineMode;
use crate::registry::RegistryProtocol;
use crate::resolve::types::ResolutionGraph;
use reqwest::Client;
use std::collections::BTreeMap;
use tokio::sync::mpsc;

pub(crate) use registry::prefetch_registry_request;
pub(crate) use state::ResolverState;

#[derive(Clone, Debug)]
pub(in crate::resolve) struct RegistryPrefetchRequest {
    pub(in crate::resolve) cache_key: String,
    pub(in crate::resolve) source: String,
    pub(in crate::resolve) protocol: RegistryProtocol,
}

#[derive(Clone)]
pub(in crate::resolve) struct ResolverContext<'a> {
    pub(in crate::resolve) config: &'a SnpmConfig,
    pub(in crate::resolve) client: &'a Client,
    pub(in crate::resolve) min_age_days: Option<u32>,
    pub(in crate::resolve) force: bool,
    pub(in crate::resolve) overrides: Option<&'a BTreeMap<String, String>>,
    pub(in crate::resolve) workspace_sources: Option<&'a BTreeMap<String, String>>,
    pub(in crate::resolve) existing_graph: Option<&'a ResolutionGraph>,
    pub(in crate::resolve) offline_mode: OfflineMode,
    pub(super) state: ResolverState,
    pub(super) prefetch_tx: mpsc::UnboundedSender<ResolvedPackage>,
    pub(super) metadata_prefetch_tx: mpsc::UnboundedSender<RegistryPrefetchRequest>,
}

#[cfg(test)]
impl<'a> ResolverContext<'a> {
    pub(crate) fn new_for_tests(
        config: &'a SnpmConfig,
        client: &'a Client,
        existing_graph: Option<&'a ResolutionGraph>,
        min_age_days: Option<u32>,
        force: bool,
        overrides: Option<&'a BTreeMap<String, String>>,
        workspace_sources: Option<&'a BTreeMap<String, String>>,
        offline_mode: OfflineMode,
    ) -> Self {
        let state = ResolverState::new(64);
        let (prefetch_tx, _prefetch_rx) = mpsc::unbounded_channel();
        let (metadata_prefetch_tx, _metadata_prefetch_rx) = mpsc::unbounded_channel();

        Self {
            config,
            client,
            min_age_days,
            force,
            overrides,
            workspace_sources,
            existing_graph,
            offline_mode,
            state,
            prefetch_tx,
            metadata_prefetch_tx,
        }
    }
}
