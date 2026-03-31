use super::super::super::types::{PackageId, ResolvedPackage};
use super::ResolverContext;

use std::collections::BTreeMap;

impl<'a> ResolverContext<'a> {
    pub(in crate::resolve) async fn package_already_resolved(&self, id: &PackageId) -> bool {
        self.state.packages.lock().await.contains_key(id)
    }

    pub(in crate::resolve) async fn insert_placeholder_if_missing(
        &self,
        package: ResolvedPackage,
    ) -> bool {
        let mut packages = self.state.packages.lock().await;
        if packages.contains_key(&package.id) {
            return false;
        }

        packages.insert(package.id.clone(), package);
        true
    }

    pub(in crate::resolve) async fn update_dependencies(
        &self,
        id: &PackageId,
        dependencies: BTreeMap<String, PackageId>,
    ) {
        if let Some(package) = self.state.packages.lock().await.get_mut(id) {
            package.dependencies = dependencies;
        }
    }

    pub(in crate::resolve) fn notify_prefetch(&self, package: ResolvedPackage) {
        let _ = self.prefetch_tx.send(package);
    }
}
