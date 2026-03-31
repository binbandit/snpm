mod paths;
mod protocol;
mod range;

pub(in crate::resolve) use paths::resolve_relative_path;
pub(in crate::resolve) use protocol::protocol_from_range;
pub(in crate::resolve) use range::normalize_dependency_range;

#[cfg(test)]
mod tests;
