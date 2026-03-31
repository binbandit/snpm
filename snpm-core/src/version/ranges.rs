use crate::{Result, SnpmError};
use snpm_semver::RangeSet;

pub fn parse_range_set(name: &str, original: &str) -> Result<RangeSet> {
    RangeSet::parse(original).map_err(|err| SnpmError::Semver {
        value: format!("{}@{}", name, original),
        reason: err.to_string(),
    })
}
