use crate::registry::RegistryPackage;
use crate::registry::RegistryVersion;
use crate::{Result, SnpmError};
use snpm_semver::{RangeSet, Version};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

pub fn select_version(
    name: &str,
    range: &str,
    package: &RegistryPackage,
    min_age_days: Option<u32>,
    force: bool,
) -> Result<RegistryVersion> {
    let trimmed = range.trim();

    if let Some(tag_version) = package.dist_tags.get(trimmed)
        && let Some(meta) = package.versions.get(tag_version)
    {
        let now = OffsetDateTime::now_utc();

        if let Some(min_days) = min_age_days
            && !force
            && let Some(age_days) = version_age_days(package, &meta.version, now)
            && age_days < min_days as i64
        {
            return Err(SnpmError::ResolutionFailed {
                name: name.to_string(),
                range: range.to_string(),
                reason: format!(
                    "dist-tag {} points to version {} which is only {} days old, less than the configured minimum of {} days",
                    range, meta.version, age_days, min_days
                ),
            });
        }

        return Ok(meta.clone());
    }
    let ranges = parse_range_set(name, range)?;
    let mut selected: Option<(Version, RegistryVersion)> = None;
    let now = OffsetDateTime::now_utc();
    let mut youngest_rejected: Option<(String, i64)> = None;

    for (version_str, meta) in package.versions.iter() {
        let parsed = Version::parse(version_str);
        if let Ok(ver) = parsed {
            if !ranges.matches(&ver) {
                continue;
            }

            if let Some(min_days) = min_age_days
                && !force
                && let Some(age_days) = version_age_days(package, version_str, now)
                && age_days < min_days as i64
            {
                if youngest_rejected.is_none() {
                    youngest_rejected = Some((version_str.clone(), age_days));
                }
                continue;
            }

            match &selected {
                Some((best, _)) if ver <= *best => {}
                _ => selected = Some((ver, meta.clone())),
            }
        }
    }

    if let Some((_, meta)) = selected {
        Ok(meta)
    } else {
        if let Some(min_days) = min_age_days
            && !force
            && let Some((ver_str, age_days)) = youngest_rejected
        {
            return Err(SnpmError::ResolutionFailed {
                name: name.to_string(),
                range: range.to_string(),
                reason: format!(
                    "latest matching version {ver_str} is only {age_days} days old, which is less than the configured minimum of {min_days} days"
                ),
            });
        }

        Err(SnpmError::ResolutionFailed {
            name: name.to_string(),
            range: range.to_string(),
            reason: "Version not found matching range".to_string(),
        })
    }
}

pub fn parse_range_set(name: &str, original: &str) -> Result<RangeSet> {
    RangeSet::parse(original).map_err(|err| SnpmError::Semver {
        value: format!("{}@{}", name, original),
        reason: err.to_string(),
    })
}

fn version_age_days(package: &RegistryPackage, version: &str, now: OffsetDateTime) -> Option<i64> {
    let time_val = package.time.get(version)?;
    let time_str = time_val.as_str()?;
    let published = OffsetDateTime::parse(time_str, &Rfc3339).ok()?;
    let age = now - published;
    Some(age.whole_days())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_matches_simple_range() {
        let ranges = parse_range_set("pkg", ">= 4.21.0").unwrap();
        let v = Version::parse("4.21.0").unwrap();
        assert!(ranges.matches(&v));
    }

    #[test]
    fn selects_custom_dist_tag() {
        let mut versions = std::collections::BTreeMap::new();
        let version_meta = RegistryVersion {
            version: "1.0.0".to_string(),
            dependencies: Default::default(),
            optional_dependencies: Default::default(),
            peer_dependencies: Default::default(),
            peer_dependencies_meta: Default::default(),
            bundled_dependencies: None,
            bundle_dependencies: None,
            dist: crate::registry::RegistryDist {
                tarball: "Url".to_string(),
                integrity: None,
            },
            os: vec![],
            cpu: vec![],
            bin: None,
        };
        versions.insert("1.0.0".to_string(), version_meta);

        let mut dist_tags = std::collections::BTreeMap::new();
        dist_tags.insert("ts5.9".to_string(), "1.0.0".to_string());

        let package = RegistryPackage {
            versions,
            time: Default::default(),
            dist_tags,
        };

        // Should succeed resolving "ts5.9" tag
        let result = select_version("pkg", "ts5.9", &package, None, false);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().version, "1.0.0");
    }
}
