use super::super::super::types::{PackageId, ResolvedPackage};
use crate::registry::RegistryPackage;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock, Semaphore};

type PackageMap = BTreeMap<PackageId, ResolvedPackage>;
type PackageCache = BTreeMap<String, Arc<RegistryPackage>>;
type PackageFetchLocks = BTreeMap<String, Arc<Mutex<()>>>;

#[derive(Clone)]
pub(crate) struct ResolverState {
    pub(super) packages: Arc<Mutex<PackageMap>>,
    pub(super) package_cache: Arc<RwLock<PackageCache>>,
    pub(super) package_fetch_locks: Arc<Mutex<PackageFetchLocks>>,
    pub(super) prefetched_registry_keys: Arc<Mutex<BTreeSet<String>>>,
    pub(super) registry_semaphore: Arc<Semaphore>,
}

impl ResolverState {
    pub(crate) fn new(concurrency: usize) -> Self {
        Self {
            packages: Arc::new(Mutex::new(PackageMap::new())),
            package_cache: Arc::new(RwLock::new(PackageCache::new())),
            package_fetch_locks: Arc::new(Mutex::new(PackageFetchLocks::new())),
            prefetched_registry_keys: Arc::new(Mutex::new(BTreeSet::new())),
            registry_semaphore: Arc::new(Semaphore::new(concurrency)),
        }
    }

    pub(crate) async fn take_packages(&self) -> PackageMap {
        let mut packages = self.packages.lock().await;
        std::mem::take(&mut *packages)
    }
}
