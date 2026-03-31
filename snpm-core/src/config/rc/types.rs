use crate::config::{AuthScheme, HoistingMode};
use std::collections::BTreeMap;

#[derive(Default)]
pub struct RegistryConfig {
    pub default_registry: String,
    pub scoped: BTreeMap<String, String>,
    pub registry_auth: BTreeMap<String, String>,
    pub registry_auth_schemes: BTreeMap<String, AuthScheme>,
    pub default_auth_token: Option<String>,
    pub hoisting: Option<HoistingMode>,
    pub default_auth_basic: bool,
    pub always_auth: bool,
}
