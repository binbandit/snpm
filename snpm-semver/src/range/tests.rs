use super::parse::{is_plain_exact_version, normalize_and_part};
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
    let version = Version::parse("4.21.0").unwrap();
    assert!(set.matches(&version));
}

#[test]
fn treats_latest_as_wildcard() {
    let set = RangeSet::parse("latest").unwrap();
    let version = Version::parse("999.0.0").unwrap();
    assert!(set.matches(&version));
}

#[test]
fn handles_or_ranges() {
    let set = RangeSet::parse("^1.0.0 || ^2.0.0").unwrap();
    assert!(set.matches(&Version::parse("1.5.0").unwrap()));
    assert!(set.matches(&Version::parse("2.3.0").unwrap()));
    assert!(!set.matches(&Version::parse("3.0.0").unwrap()));
}

#[test]
fn bare_version_is_exact_like_node_semver() {
    let set = RangeSet::parse("7.2.5").unwrap();
    assert!(set.matches(&Version::parse("7.2.5").unwrap()));
    assert!(!set.matches(&Version::parse("7.2.7").unwrap()));
}

#[test]
fn alias_latest_still_parses() {
    let set = RangeSet::parse("npm:rolldown-vite@7.2.5").unwrap();
    assert!(set.matches(&Version::parse("7.2.5").unwrap()));
    assert!(!set.matches(&Version::parse("7.2.7").unwrap()));
}

#[test]
fn hyphen_range_major_only() {
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
    let set = RangeSet::parse("1.2 - 2.3").unwrap();
    assert!(set.matches(&Version::parse("1.2.0").unwrap()));
    assert!(set.matches(&Version::parse("1.9.0").unwrap()));
    assert!(set.matches(&Version::parse("2.3.9").unwrap()));
    assert!(!set.matches(&Version::parse("1.1.9").unwrap()));
    assert!(!set.matches(&Version::parse("2.4.0").unwrap()));
}

#[test]
fn hyphen_range_full_versions() {
    let set = RangeSet::parse("1.2.3 - 2.3.4").unwrap();
    assert!(set.matches(&Version::parse("1.2.3").unwrap()));
    assert!(set.matches(&Version::parse("2.3.4").unwrap()));
    assert!(set.matches(&Version::parse("2.0.0").unwrap()));
    assert!(!set.matches(&Version::parse("1.2.2").unwrap()));
    assert!(!set.matches(&Version::parse("2.3.5").unwrap()));
}

#[test]
fn empty_string_matches_everything() {
    let set = RangeSet::parse("").unwrap();
    assert!(set.matches(&Version::parse("1.0.0").unwrap()));
}

#[test]
fn wildcard_matches_everything() {
    let set = RangeSet::parse("*").unwrap();
    assert!(set.matches(&Version::parse("0.0.1").unwrap()));
    assert!(set.matches(&Version::parse("999.999.999").unwrap()));
}

#[test]
fn caret_range_zero_major() {
    let set = RangeSet::parse("^0.2.3").unwrap();
    assert!(set.matches(&Version::parse("0.2.3").unwrap()));
    assert!(set.matches(&Version::parse("0.2.9").unwrap()));
    assert!(!set.matches(&Version::parse("0.3.0").unwrap()));
    assert!(!set.matches(&Version::parse("0.2.2").unwrap()));
}

#[test]
fn tilde_range() {
    let set = RangeSet::parse("~1.2.3").unwrap();
    assert!(set.matches(&Version::parse("1.2.3").unwrap()));
    assert!(set.matches(&Version::parse("1.2.99").unwrap()));
    assert!(!set.matches(&Version::parse("1.3.0").unwrap()));
}

#[test]
fn greater_than() {
    let set = RangeSet::parse(">1.0.0").unwrap();
    assert!(!set.matches(&Version::parse("1.0.0").unwrap()));
    assert!(set.matches(&Version::parse("1.0.1").unwrap()));
    assert!(set.matches(&Version::parse("2.0.0").unwrap()));
}

#[test]
fn less_than() {
    let set = RangeSet::parse("<2.0.0").unwrap();
    assert!(set.matches(&Version::parse("1.9.9").unwrap()));
    assert!(!set.matches(&Version::parse("2.0.0").unwrap()));
}

#[test]
fn less_than_or_equal() {
    let set = RangeSet::parse("<=2.0.0").unwrap();
    assert!(set.matches(&Version::parse("2.0.0").unwrap()));
    assert!(set.matches(&Version::parse("1.9.9").unwrap()));
    assert!(!set.matches(&Version::parse("2.0.1").unwrap()));
}

#[test]
fn complex_or_range() {
    let set = RangeSet::parse(">=1.0.0 <2.0.0 || >=3.0.0 <4.0.0").unwrap();
    assert!(set.matches(&Version::parse("1.5.0").unwrap()));
    assert!(!set.matches(&Version::parse("2.5.0").unwrap()));
    assert!(set.matches(&Version::parse("3.5.0").unwrap()));
    assert!(!set.matches(&Version::parse("4.0.0").unwrap()));
}

#[test]
fn prerelease_exact_match() {
    let set = RangeSet::parse("1.0.0-alpha.1").unwrap();
    assert!(set.matches(&Version::parse("1.0.0-alpha.1").unwrap()));
    assert!(!set.matches(&Version::parse("1.0.0-alpha.2").unwrap()));
    assert!(!set.matches(&Version::parse("1.0.0").unwrap()));
}

#[test]
fn npm_protocol_scoped_package() {
    let set = RangeSet::parse("npm:@scope/pkg@^1.0.0").unwrap();
    assert!(set.matches(&Version::parse("1.5.0").unwrap()));
    assert!(!set.matches(&Version::parse("2.0.0").unwrap()));
}

#[test]
fn npm_protocol_unscoped_package() {
    let set = RangeSet::parse("npm:other-name@~2.3.0").unwrap();
    assert!(set.matches(&Version::parse("2.3.5").unwrap()));
    assert!(!set.matches(&Version::parse("2.4.0").unwrap()));
}

#[test]
fn jsr_protocol() {
    let set = RangeSet::parse("jsr:@std/path@^1.0.0").unwrap();
    assert!(set.matches(&Version::parse("1.2.3").unwrap()));
    assert!(!set.matches(&Version::parse("2.0.0").unwrap()));
}

#[test]
fn npm_protocol_no_version() {
    let set = RangeSet::parse("npm:some-pkg").unwrap();
    assert!(set.matches(&Version::parse("999.0.0").unwrap()));
}

#[test]
fn is_plain_exact_version_true() {
    assert!(is_plain_exact_version("1.2.3"));
    assert!(is_plain_exact_version("0.0.0"));
    assert!(is_plain_exact_version("1.2.3-alpha"));
}

#[test]
fn is_plain_exact_version_false() {
    assert!(!is_plain_exact_version("^1.2.3"));
    assert!(!is_plain_exact_version("~1.2.3"));
    assert!(!is_plain_exact_version(">=1.2.3"));
    assert!(!is_plain_exact_version("1.x.0"));
    assert!(!is_plain_exact_version("*"));
    assert!(!is_plain_exact_version(""));
    assert!(!is_plain_exact_version("1.2"));
}
