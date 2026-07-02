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

        // Identity is the *resolved* package, not the edge name: an
        // aliased dep ("foo": "npm:bar@^1") resolves to bar's content
        // and must be keyed as bar. Keying by the edge name would
        // collide with a real package also called "foo" at the same
        // version — whichever resolved first would silently serve the
        // wrong tarball to the other consumer. Edge names live in the
        // parent's dependency map (and the lockfile root's `package`
        // field), so nothing is lost. Git/file/custom sources keep the
        // edge name: their `source` is a URL or path, not a name.
        let resolved_name =
            if matches!(request.protocol.name.as_str(), "npm" | "jsr") && request.source != name {
                request.source.clone()
            } else {
                name.to_string()
            };

        let id = PackageId {
            name: resolved_name,
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
        self.prefetch_dependency_frontier(&version_meta).await;

        // If the dependency subtree fails to resolve, drop the
        // placeholder before propagating: under an optional edge the
        // caller swallows the error, and a surviving placeholder with
        // empty `dependencies` would be persisted to the lockfile as a
        // complete package — installed forever without its transitive
        // deps. A removed entry at worst leaves a dangling edge in a
        // concurrent consumer, which the next install detects as an
        // incomplete subgraph and re-resolves.
        let dependencies = match self.resolve_dependencies(&id, &version_meta).await {
            Ok(dependencies) => dependencies,
            Err(error) => {
                self.remove_package(&id).await;
                return Err(error);
            }
        };
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

        let seed_id = candidate?;

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
