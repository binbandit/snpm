use crate::config::rc::host_from_url;
use crate::config::{AuthScheme, SnpmConfig};

impl SnpmConfig {
    pub fn registry_url_for_package_name(&self, name: &str) -> String {
        if let Some((scope, _)) = name.split_once('/')
            && scope.starts_with('@')
            && let Some(registry) = self.scoped_registries.get(scope)
        {
            return registry.clone();
        }
        self.default_registry.clone()
    }

    pub fn authorization_header_for_tarball(
        &self,
        package_name: &str,
        tarball_url: &str,
    ) -> Option<String> {
        let registry_url = self.registry_url_for_package_name(package_name);
        let registry_host = host_from_url(&registry_url)?;
        let tarball_host = host_from_url(tarball_url)?;
        if registry_host != tarball_host {
            return None;
        }
        self.authorization_header_for_url(tarball_url)
    }

    pub fn auth_token_for_url(&self, url: &str) -> Option<&str> {
        let host = host_from_url(url)?;

        if let Some(token) = self.registry_auth.get(&host) {
            return Some(token.as_str());
        }

        if let Some(default_host) = host_from_url(&self.default_registry)
            && host == default_host
            && let Some(token) = self.default_registry_auth_token.as_ref()
        {
            return Some(token.as_str());
        }

        None
    }

    pub fn auth_scheme_for_url(&self, url: &str) -> AuthScheme {
        if let Some(host) = host_from_url(url)
            && let Some(scheme) = self.registry_auth_schemes.get(&host)
        {
            return *scheme;
        }

        self.default_registry_auth_scheme
    }

    pub fn authorization_header_for_url(&self, url: &str) -> Option<String> {
        let token = self.auth_token_for_url(url)?;
        let scheme = self.auth_scheme_for_url(url);

        Some(match scheme {
            AuthScheme::Bearer => format!("Bearer {}", token),
            AuthScheme::Basic => format!("Basic {}", token),
        })
    }
}
