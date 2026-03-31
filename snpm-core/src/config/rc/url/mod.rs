mod host;
mod normalize;

pub(crate) use host::host_from_url;
pub use normalize::normalize_registry_url;

#[cfg(test)]
mod tests;
