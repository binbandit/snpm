use super::super::super::types::{PackageId, ResolvedPackage};
use crate::registry::RegistryPackage;

use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock, Semaphore};

type PackageMap = BTreeMap<PackageId, ResolvedPackage>;
type PackageCache = BTreeMap<String, Arc<RegistryPackage>>;

#[derive(Clone)]
pub(in crate::resolve::engine) struct ResolverState {
    pub(super) packages: Arc<Mutex<PackageMap>>,
    pub(super) package_cache: Arc<RwLock<PackageCache>>,
    pub(super) registry_semaphore: Arc<Semaphore>,
}

impl ResolverState {
    pub(in crate::resolve::engine) fn new(concurrency: usize) -> Self {
        Self {
            packages: Arc::new(Mutex::new(PackageMap::new())),
            package_cache: Arc::new(RwLock::new(PackageCache::new())),
            registry_semaphore: Arc::new(Semaphore::new(concurrency)),
        }
    }

    pub(in crate::resolve::engine) async fn take_packages(&self) -> PackageMap {
        let mut packages = self.packages.lock().await;
        std::mem::take(&mut *packages)
    }
}
