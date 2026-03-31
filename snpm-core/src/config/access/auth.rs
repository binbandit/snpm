use crate::config::rc::host_from_url;
use crate::config::{AuthScheme, SnpmConfig};

impl SnpmConfig {
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
