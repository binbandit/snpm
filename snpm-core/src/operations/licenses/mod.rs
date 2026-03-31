mod collect;
mod parse;

pub use collect::collect_licenses;
pub use parse::LicenseEntry;

#[cfg(test)]
mod tests;
