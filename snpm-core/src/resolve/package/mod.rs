mod dependencies;
mod metadata;

use super::engine::ResolverContext;
use super::query::build_dep_request;
use super::types::PackageId;
use crate::Result;
use crate::registry::RegistryProtocol;
use crate::version::select_version;
use async_recursion::async_recursion;

use metadata::{build_placeholder, ensure_platform_compatible};

impl<'a> ResolverContext<'a> {
    #[async_recursion]
    pub(super) async fn resolve_package(
        &self,
        name: &str,
        range: &str,
        protocol: &RegistryProtocol,
    ) -> Result<PackageId> {
        let request = build_dep_request(name, range, protocol, self.overrides);
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

        let dependencies = self.resolve_dependencies(&version_meta).await?;
        self.update_dependencies(&id, dependencies).await;

        Ok(id)
    }
}
