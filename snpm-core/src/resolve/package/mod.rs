mod dependencies;
mod metadata;
#[cfg(test)]
mod tests;

use super::engine::ResolverContext;
use super::query::build_dep_request;
use super::types::{PackageId, ResolutionGraph};
use crate::Result;
use crate::registry::RegistryProtocol;
use crate::version::{parse_range_set, select_version};
use async_recursion::async_recursion;
use snpm_semver::parse_version;
use std::collections::BTreeSet;

use metadata::{build_placeholder, ensure_platform_compatible};

impl<'a> ResolverContext<'a> {
    #[async_recursion]
    pub(super) async fn resolve_package(
        &self,
        name: &str,
        range: &str,
        protocol: &RegistryProtocol,
        parent_id: Option<&PackageId>,
    ) -> Result<PackageId> {
        if let Some(existing_graph) = self.existing_graph
            && let Some(seed_id) = self.seeded_dependency_id(name, range, parent_id, existing_graph)
            && self.seeded_subgraph_complete(seed_id.clone(), existing_graph)
            && self
                .import_seed_package_chain(seed_id.clone(), existing_graph)
                .await?
        {
            return Ok(seed_id);
        }

        let request = build_dep_request(
            name,
            range,
            protocol,
            self.overrides,
            self.workspace_sources,
        );
        let cache_key = format!("{}:{}", request.protocol.name, request.source);
        let package = self
            .fetch_registry_package(&cache_key, &request.source, &request.protocol)
            .await?;

        let version_meta = select_version(
            &request.source,
            &request.range,
            &package,
            self.min_age_days,
            self.force,
        )?;

        ensure_platform_compatible(name, range, &version_meta)?;

        let id = PackageId {
            name: name.to_string(),
            version: version_meta.version.clone(),
        };

        if self.package_already_resolved(&id).await {
            return Ok(id);
        }

        let placeholder = build_placeholder(&id, &version_meta);
        if !self
            .insert_placeholder_if_missing(placeholder.clone())
            .await
        {
            return Ok(id);
        }

        self.notify_prefetch(placeholder);

        let dependencies = self.resolve_dependencies(&id, &version_meta).await?;
        self.update_dependencies(&id, dependencies).await;

        Ok(id)
    }

    fn seeded_dependency_id(
        &self,
        name: &str,
        range: &str,
        parent_id: Option<&PackageId>,
        graph: &'a ResolutionGraph,
    ) -> Option<PackageId> {
        let candidate = match parent_id {
            Some(parent_id) => graph
                .packages
                .get(parent_id)?
                .dependencies
                .get(name)
                .cloned(),
            None => graph
                .root
                .dependencies
                .get(name)
                .map(|dep| dep.resolved.clone()),
        };

        let Some(seed_id) = candidate else {
            return None;
        };

        let parsed_range = parse_range_set(name, range).ok()?;
        let parsed_version = parse_version(&seed_id.version).ok()?;

        if !parsed_range.matches(&parsed_version) {
            return None;
        }

        Some(seed_id)
    }

    fn seeded_subgraph_complete(&self, package_id: PackageId, graph: &'a ResolutionGraph) -> bool {
        let mut stack = vec![package_id];
        let mut seen = BTreeSet::new();

        while let Some(current) = stack.pop() {
            if !seen.insert(current.clone()) {
                continue;
            }

            let Some(package) = graph.packages.get(&current) else {
                return false;
            };

            for dependency_id in package.dependencies.values() {
                stack.push(dependency_id.clone());
            }
        }

        true
    }

    async fn import_seed_package_chain(
        &self,
        package_id: PackageId,
        graph: &'a ResolutionGraph,
    ) -> Result<bool> {
        let mut stack = vec![package_id];
        let mut inserted = BTreeSet::new();

        while let Some(current_id) = stack.pop() {
            if !inserted.insert(current_id.clone()) {
                continue;
            }

            let Some(package) = graph.packages.get(&current_id).cloned() else {
                return Ok(false);
            };

            if self.insert_placeholder_if_missing(package.clone()).await {
                self.notify_prefetch(package.clone());
            }

            for dependency_id in package.dependencies.values() {
                stack.push(dependency_id.clone());
            }
        }

        Ok(true)
    }
}
