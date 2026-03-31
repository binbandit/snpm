use semver::Version;

pub fn parse_version(input: &str) -> Result<Version, semver::Error> {
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

    for (index, segment) in version_part.split('.').enumerate() {
        if index > 0 {
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
    use crate::RangeSet;

    #[test]
    fn parse_version_strips_leading_zeros() {
        let version = parse_version("30.100.00").unwrap();
        assert_eq!(version.major, 30);
        assert_eq!(version.minor, 100);
        assert_eq!(version.patch, 0);
    }

    #[test]
    fn parse_version_strips_leading_zeros_all_components() {
        let version = parse_version("01.02.03").unwrap();
        assert_eq!(version.major, 1);
        assert_eq!(version.minor, 2);
        assert_eq!(version.patch, 3);
    }

    #[test]
    fn parse_version_handles_all_zeros() {
        let version = parse_version("00.00.00").unwrap();
        assert_eq!(version.major, 0);
        assert_eq!(version.minor, 0);
        assert_eq!(version.patch, 0);
    }

    #[test]
    fn parse_version_preserves_prerelease() {
        let version = parse_version("01.02.03-alpha.1").unwrap();
        assert_eq!(version.major, 1);
        assert_eq!(version.minor, 2);
        assert_eq!(version.patch, 3);
        assert_eq!(version.pre.as_str(), "alpha.1");
    }

    #[test]
    fn parse_version_preserves_build_metadata() {
        let version = parse_version("01.02.03+build.42").unwrap();
        assert_eq!(version.major, 1);
        assert_eq!(version.minor, 2);
        assert_eq!(version.patch, 3);
        assert_eq!(version.build.as_str(), "build.42");
    }

    #[test]
    fn parse_version_strict_still_works() {
        let version = parse_version("1.2.3").unwrap();
        assert_eq!(version.major, 1);
        assert_eq!(version.minor, 2);
        assert_eq!(version.patch, 3);
    }

    #[test]
    fn parse_version_matches_range_after_normalization() {
        let set = RangeSet::parse("^30.100.0").unwrap();
        let version = parse_version("30.100.00").unwrap();
        assert!(set.matches(&version));
    }

    #[test]
    fn parse_version_v_prefix() {
        let version = parse_version("v1.2.3").unwrap();
        assert_eq!(version.major, 1);
        assert_eq!(version.minor, 2);
        assert_eq!(version.patch, 3);
    }

    #[test]
    fn parse_version_uppercase_v_prefix() {
        let version = parse_version("V1.2.3").unwrap();
        assert_eq!(version.major, 1);
    }
}
