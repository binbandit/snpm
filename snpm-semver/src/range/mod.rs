mod parse;

use semver::{Version, VersionReq};

use crate::Error;

#[derive(Debug, Clone)]
pub struct RangeSet {
    original: String,
    ranges: Vec<VersionReq>,
}

impl RangeSet {
    pub fn parse(original: &str) -> Result<Self, Error> {
        parse::parse_internal(original)
    }

    pub fn matches(&self, version: &Version) -> bool {
        self.ranges.iter().any(|range| range.matches(version))
    }

    pub fn original(&self) -> &str {
        &self.original
    }
}

#[cfg(test)]
mod tests;
