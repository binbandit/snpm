use super::ResolverContext;
use crate::Result;
use crate::registry::{RegistryPackage, RegistryProtocol};

use std::sync::Arc;

impl<'a> ResolverContext<'a> {
    pub(in crate::resolve) async fn fetch_registry_package(
        &self,
        cache_key: &str,
        source: &str,
        protocol: &RegistryProtocol,
    ) -> Result<Arc<RegistryPackage>> {
        if let Some(package) = self
            .state
            .package_cache
            .read()
            .await
            .get(cache_key)
            .cloned()
        {
            return Ok(package);
        }

        let _permit = self.state.registry_semaphore.acquire().await.unwrap();

        if let Some(package) = self
            .state
            .package_cache
            .read()
            .await
            .get(cache_key)
            .cloned()
        {
            return Ok(package);
        }

        let fetched = crate::registry::fetch_package_with_offline(
            self.config,
            self.client,
            source,
            protocol,
            self.offline_mode,
        )
        .await?;

        let fetched = Arc::new(fetched);
        self.state
            .package_cache
            .write()
            .await
            .insert(cache_key.to_string(), fetched.clone());

        Ok(fetched)
    }
}
