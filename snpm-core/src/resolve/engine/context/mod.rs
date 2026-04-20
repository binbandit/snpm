mod packages;
mod registry;
mod root;
mod state;

use super::super::types::ResolvedPackage;
use crate::SnpmConfig;
use crate::config::OfflineMode;
use crate::resolve::types::ResolutionGraph;
use reqwest::Client;
use std::collections::BTreeMap;
use tokio::sync::mpsc;

pub(crate) use state::ResolverState;

#[derive(Clone)]
pub(in crate::resolve) struct ResolverContext<'a> {
    pub(in crate::resolve) config: &'a SnpmConfig,
    pub(in crate::resolve) client: &'a Client,
    pub(in crate::resolve) min_age_days: Option<u32>,
    pub(in crate::resolve) force: bool,
    pub(in crate::resolve) overrides: Option<&'a BTreeMap<String, String>>,
    pub(in crate::resolve) existing_graph: Option<&'a ResolutionGraph>,
    pub(in crate::resolve) offline_mode: OfflineMode,
    pub(super) state: ResolverState,
    pub(super) prefetch_tx: mpsc::UnboundedSender<ResolvedPackage>,
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
        offline_mode: OfflineMode,
    ) -> Self {
        let state = ResolverState::new(64);
        let (prefetch_tx, _prefetch_rx) = mpsc::unbounded_channel();

        Self {
            config,
            client,
            min_age_days,
            force,
            overrides,
            existing_graph,
            offline_mode,
            state,
            prefetch_tx,
        }
    }
}
