use semver::VersionReq;
use std::error::Error as StdError;
use std::fmt;

#[derive(Debug, Clone)]
pub struct RangeSet {
    original: String,
    ranges: Vec<VersionReq>,
}

#[derive(Debug, Clone)]
pub struct Error {
    input: String,
    message: String,
}

impl Error {
    pub fn new(input: String, message: String) -> Self {
        Self { input, message }
    }

    pub fn input(&self) -> &str {
        &self.input
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.message, self.input)
    }
}

impl StdError for Error {}

impl RangeSet {
    pub fn parse(original: &str) -> Result<Self, Error> {
        parse_internal(original)
    }

    pub fn matches(&self, version: &Version) -> bool {
        self.ranges.iter().any(|r| r.matches(version))
    }

    pub fn original(&self) -> &str {
        &self.original
    }
}

fn parse_internal(original: &str) -> Result<RangeSet, Error> {
    let mut s = original.trim();

    if s.is_empty() || s == "latest" {
        s = "*";
    }

    if s.starts_with("npm:") || s.starts_with("jsr:") {
        if let Some(colon) = s.find(':') {
            let after = &s[colon + 1..];
            if let Some(at) = after.rfind('@') {
                let version = after[at + 1..].trim();
                if version.is_empty() {
                    s = "*";
                } else {
                    s = version;
                }
            } else {
                s = "*";
            }
        }
    }

    let mut ranges = Vec::new();

    for part in s.split("||") {
        let mut part = part.trim();
        if part.is_empty() {
            continue;
        }

        if part.starts_with('@') && part.len() > 1 {
            let second = part.as_bytes()[1] as char;
            if second.is_ascii_digit() || matches!(second, '^' | '~' | '>' | '<' | '=') {
                part = &part[1..];
            }
        }

        if part.is_empty() || part == "latest" {
            part = "*"
        }

        let normalized = normalize_and_part(part);

        let req = VersionReq::parse(&normalized)
            .map_err(|err| Error::new(original.to_string(), err.to_string()))?;

        ranges.push(req);
    }

    if ranges.is_empty() {
        let req = VersionReq::parse("*")
            .map_err(|err| Error::new(original.to_string(), err.to_string()))?;
        ranges.push(req);
    }

    Ok(RangeSet {
        original: original.to_string(),
        ranges,
    })
}

fn normalize_and_part(part: &str) -> String {
    let tokens: Vec<&str> = part.split_whitespace().collect();

    if tokens.len() <= 1 {
        return part.to_string();
    }

    if tokens.len() == 3 && tokens[1] == "-" {
        return part.to_string();
    }

    let mut result = String::new();
    for (i, token) in tokens.iter().enumerate() {
        if i > 0 {
            let prev = tokens[i - 1];
            if matches!(prev, "=" | ">" | ">=" | "<" | "<=" | "~" | "^") {
                result.push(' ');
            } else {
                result.push_str(", ");
            }
        }

        result.push_str(token);
    }
    result
}

pub use semver::Version;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_ge_space() {
        let input = ">= 4.21.0";
        let normalized = normalize_and_part(input);
        let req = VersionReq::parse(&normalized);
        assert!(
            req.is_ok(),
            "Failed to parse normalized '{}' -> '{}': {:?}",
            input,
            normalized,
            req.err()
        );
    }

    #[test]
    fn parses_simple_range() {
        let set = RangeSet::parse(">= 4.21.0").unwrap();
        let v = Version::parse("4.21.0").unwrap();
        assert!(set.matches(&v));
    }

    #[test]
    fn treats_latest_as_wildcard() {
        let set = RangeSet::parse("latest").unwrap();
        let v = Version::parse("999.0.0").unwrap();
        assert!(set.matches(&v));
    }

    #[test]
    fn handles_or_ranges() {
        let set = RangeSet::parse("^1.0.0 || ^2.0.0").unwrap();
        let v1 = Version::parse("1.5.0").unwrap();
        let v2 = Version::parse("2.3.0").unwrap();
        let v3 = Version::parse("3.0.0").unwrap();
        assert!(set.matches(&v1));
        assert!(set.matches(&v2));
        assert!(!set.matches(&v3));
    }
}
