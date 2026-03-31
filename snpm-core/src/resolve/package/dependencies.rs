use super::super::engine::ResolverContext;
use super::super::source::{normalize_dependency_range, protocol_from_range};
use super::super::types::PackageId;
use crate::SnpmError;
use crate::registry::RegistryVersion;

use futures::future::{join, join_all};
use std::collections::{BTreeMap, BTreeSet};

use super::metadata::bundled_dependency_names;

impl<'a> ResolverContext<'a> {
    pub(super) async fn resolve_dependencies(
        &self,
        version_meta: &RegistryVersion,
    ) -> crate::Result<BTreeMap<String, PackageId>> {
        let bundled = bundled_dependency_names(version_meta);
        let (required, optional) = join(
            self.resolve_required_dependencies(version_meta, &bundled),
            self.resolve_optional_dependencies(version_meta, &bundled),
        )
        .await;

        let mut dependencies = required?;
        for (name, id) in optional {
            dependencies.insert(name, id);
        }

        Ok(dependencies)
    }

    async fn resolve_required_dependencies(
        &self,
        version_meta: &RegistryVersion,
        bundled: &BTreeSet<String>,
    ) -> crate::Result<BTreeMap<String, PackageId>> {
        let mut futures = Vec::new();

        for (name, range) in &version_meta.dependencies {
            if bundled.contains(name) {
                continue;
            }

            let context = self.clone();
            let name = name.clone();
            let range = normalize_dependency_range(&version_meta.dist.tarball, range);
            let protocol = protocol_from_range(&range);

            futures.push(async move {
                let id = context.resolve_package(&name, &range, &protocol).await?;
                Ok::<(String, PackageId), SnpmError>((name, id))
            });
        }

        let mut dependencies = BTreeMap::new();
        for result in join_all(futures).await {
            let (name, id) = result?;
            dependencies.insert(name, id);
        }

        Ok(dependencies)
    }

    async fn resolve_optional_dependencies(
        &self,
        version_meta: &RegistryVersion,
        bundled: &BTreeSet<String>,
    ) -> Vec<(String, PackageId)> {
        let mut futures = Vec::new();

        for (name, range) in &version_meta.optional_dependencies {
            if bundled.contains(name) {
                continue;
            }

            let context = self.clone();
            let name = name.clone();
            let range = range.clone();
            let protocol = protocol_from_range(&range);

            futures.push(async move {
                match context.resolve_package(&name, &range, &protocol).await {
                    Ok(id) => Some((name, id)),
                    Err(_) => None,
                }
            });
        }

        join_all(futures).await.into_iter().flatten().collect()
    }
}
