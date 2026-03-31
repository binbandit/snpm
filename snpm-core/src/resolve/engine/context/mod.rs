mod packages;
mod registry;
mod root;
mod state;

use super::super::types::ResolvedPackage;
use crate::SnpmConfig;
use crate::config::OfflineMode;
use reqwest::Client;
use std::collections::BTreeMap;
use tokio::sync::mpsc;

pub(in crate::resolve::engine) use state::ResolverState;

#[derive(Clone)]
pub(in crate::resolve) struct ResolverContext<'a> {
    pub(in crate::resolve) config: &'a SnpmConfig,
    pub(in crate::resolve) client: &'a Client,
    pub(in crate::resolve) min_age_days: Option<u32>,
    pub(in crate::resolve) force: bool,
    pub(in crate::resolve) overrides: Option<&'a BTreeMap<String, String>>,
    pub(in crate::resolve) offline_mode: OfflineMode,
    pub(super) state: ResolverState,
    pub(super) prefetch_tx: mpsc::UnboundedSender<ResolvedPackage>,
}
