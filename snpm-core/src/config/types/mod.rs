mod config;
mod modes;

pub use config::{SnpmConfig, default_disable_global_virtual_store_for_packages};
pub use modes::{AuthScheme, HoistingMode, LinkBackend, OfflineMode};

#[cfg(test)]
mod tests;
