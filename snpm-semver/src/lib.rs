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

    if (s.starts_with("npm:") || s.starts_with("jsr:"))
        && let Some(colon) = s.find(':')
    {
        let after = &s[colon + 1..];
        // For scoped packages like @scope/pkg@^1.0.0, we need to find the
        // version separator '@' that comes after the scope. Skip the leading
        // '@' if it's a scoped package.
        let search_from = if after.starts_with('@') {
            after.find('/').map(|i| i + 1).unwrap_or(1)
        } else {
            0
        };
        if let Some(at) = after[search_from..].rfind('@') {
            let version = after[search_from + at + 1..].trim();
            if version.is_empty() {
                s = "*";
            } else {
                s = version;
            }
        } else {
            // No version separator found (e.g. "npm:@scope/pkg" or "npm:pkg")
            s = "*";
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
            part = "*";
        }

        let normalized = normalize_and_part(part);
        let adjusted = adjust_node_default(&normalized);

        let req = VersionReq::parse(&adjusted)
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
        return expand_hyphen_range(tokens[0], tokens[2]);
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

/// The `semver` crate doesn't support npm hyphen ranges, so we expand them
/// into standard comparator syntax before delegating. Partial upper bounds
/// use exclusive `<` (next increment), full upper bounds use inclusive `<=`.
fn expand_hyphen_range(lower: &str, upper: &str) -> String {
    let lower_parts: Vec<&str> = lower.split('.').collect();
    let upper_parts: Vec<&str> = upper.split('.').collect();

    let lower_full = match lower_parts.len() {
        1 => format!("{}.0.0", lower_parts[0]),
        2 => format!("{}.{}.0", lower_parts[0], lower_parts[1]),
        _ => lower.to_string(),
    };

    match upper_parts.len() {
        1 => {
            let major: u64 = upper_parts[0].parse().unwrap_or(0);
            format!(">={}, <{}.0.0", lower_full, major + 1)
        }
        2 => {
            let major = upper_parts[0];
            let minor: u64 = upper_parts[1].parse().unwrap_or(0);
            format!(">={}, <{}.{}.0", lower_full, major, minor + 1)
        }
        _ => {
            format!(">={}, <={}", lower_full, upper)
        }
    }
}

fn adjust_node_default(input: &str) -> String {
    if is_plain_exact_version(input) {
        let mut s = String::with_capacity(input.len() + 1);
        s.push('=');
        s.push_str(input);
        s
    } else {
        input.to_string()
    }
}

fn is_plain_exact_version(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }

    if s.chars()
        .any(|c| matches!(c, '<' | '>' | '=' | '^' | '~' | ' ' | ','))
    {
        return false;
    }

    if s.contains('*') {
        return false;
    }

    // Check for x/X wildcard only in version number segments (before any prerelease/build suffix).
    // Don't reject versions like "1.2.3-experimental" that contain 'x' in prerelease tags.
    let version_part = s.split(['-', '+']).next().unwrap_or(s);
    for segment in version_part.split('.') {
        if segment.eq_ignore_ascii_case("x") {
            return false;
        }
    }

    let dot_count = s.chars().filter(|&c| c == '.').count();
    dot_count >= 2
}

pub use semver::Version;

/// Some npm packages (notably AWS SDKs) publish versions with leading zeros
/// like `30.100.00` that strict SemVer rejects. This parses leniently by
/// stripping leading zeros before delegating to the `semver` crate.
pub fn parse_version(input: &str) -> Result<Version, semver::Error> {
    // Strip v/V prefix (common in npm ecosystem, e.g. git tags)
    let input = input
        .strip_prefix('v')
        .or_else(|| input.strip_prefix('V'))
        .unwrap_or(input);

    if let Ok(version) = Version::parse(input) {
        return Ok(version);
    }

    Version::parse(&normalize_leading_zeros(input))
}

fn normalize_leading_zeros(input: &str) -> String {
    let (version_part, suffix) = match input.find(['-', '+']) {
        Some(index) => (&input[..index], &input[index..]),
        None => (input, ""),
    };

    let mut result = String::with_capacity(input.len());

    for (i, segment) in version_part.split('.').enumerate() {
        if i > 0 {
            result.push('.');
        }
        let stripped = segment.trim_start_matches('0');
        result.push_str(if stripped.is_empty() { "0" } else { stripped });
    }

    result.push_str(suffix);
    result
}

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

    #[test]
    fn bare_version_is_exact_like_node_semver() {
        let set = RangeSet::parse("7.2.5").unwrap();
        let exact = Version::parse("7.2.5").unwrap();
        let higher = Version::parse("7.2.7").unwrap();
        assert!(set.matches(&exact));
        assert!(!set.matches(&higher));
    }

    #[test]
    fn alias_latest_still_parses() {
        let set = RangeSet::parse("npm:rolldown-vite@7.2.5").unwrap();
        let exact = Version::parse("7.2.5").unwrap();
        let higher = Version::parse("7.2.7").unwrap();
        assert!(set.matches(&exact));
        assert!(!set.matches(&higher));
    }

    #[test]
    fn hyphen_range_major_only() {
        // "1 - 2" means >=1.0.0 <3.0.0
        let set = RangeSet::parse("1 - 2").unwrap();
        assert!(set.matches(&Version::parse("1.0.0").unwrap()));
        assert!(set.matches(&Version::parse("1.5.3").unwrap()));
        assert!(set.matches(&Version::parse("2.0.0").unwrap()));
        assert!(set.matches(&Version::parse("2.99.99").unwrap()));
        assert!(!set.matches(&Version::parse("0.9.9").unwrap()));
        assert!(!set.matches(&Version::parse("3.0.0").unwrap()));
    }

    #[test]
    fn hyphen_range_major_minor() {
        // "1.2 - 2.3" means >=1.2.0 <2.4.0
        let set = RangeSet::parse("1.2 - 2.3").unwrap();
        assert!(set.matches(&Version::parse("1.2.0").unwrap()));
        assert!(set.matches(&Version::parse("1.9.0").unwrap()));
        assert!(set.matches(&Version::parse("2.3.9").unwrap()));
        assert!(!set.matches(&Version::parse("1.1.9").unwrap()));
        assert!(!set.matches(&Version::parse("2.4.0").unwrap()));
    }

    #[test]
    fn parse_version_strips_leading_zeros() {
        let v = parse_version("30.100.00").unwrap();
        assert_eq!(v.major, 30);
        assert_eq!(v.minor, 100);
        assert_eq!(v.patch, 0);
    }

    #[test]
    fn parse_version_strips_leading_zeros_all_components() {
        let v = parse_version("01.02.03").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
    }

    #[test]
    fn parse_version_handles_all_zeros() {
        let v = parse_version("00.00.00").unwrap();
        assert_eq!(v.major, 0);
        assert_eq!(v.minor, 0);
        assert_eq!(v.patch, 0);
    }

    #[test]
    fn parse_version_preserves_prerelease() {
        let v = parse_version("01.02.03-alpha.1").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
        assert_eq!(v.pre.as_str(), "alpha.1");
    }

    #[test]
    fn parse_version_preserves_build_metadata() {
        let v = parse_version("01.02.03+build.42").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
        assert_eq!(v.build.as_str(), "build.42");
    }

    #[test]
    fn parse_version_strict_still_works() {
        let v = parse_version("1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
    }

    #[test]
    fn parse_version_matches_range_after_normalization() {
        let set = RangeSet::parse("^30.100.0").unwrap();
        let v = parse_version("30.100.00").unwrap();
        assert!(set.matches(&v));
    }

    #[test]
    fn hyphen_range_full_versions() {
        // "1.2.3 - 2.3.4" means >=1.2.3 <=2.3.4
        let set = RangeSet::parse("1.2.3 - 2.3.4").unwrap();
        assert!(set.matches(&Version::parse("1.2.3").unwrap()));
        assert!(set.matches(&Version::parse("2.3.4").unwrap()));
        assert!(set.matches(&Version::parse("2.0.0").unwrap()));
        assert!(!set.matches(&Version::parse("1.2.2").unwrap()));
        assert!(!set.matches(&Version::parse("2.3.5").unwrap()));
    }
}
