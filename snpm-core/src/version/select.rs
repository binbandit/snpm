use crate::registry::{RegistryPackage, RegistryVersion};
use crate::{Result, SnpmError};
use snpm_semver::{Version, parse_version};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use super::ranges::parse_range_set;

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
    let mut latest_rejected: Option<(Version, String, i64)> = None;

    for (version_str, meta) in package.versions.iter() {
        let parsed = parse_version(version_str);
        if let Ok(ver) = parsed {
            if !ranges.matches(&ver) {
                continue;
            }

            if let Some(min_days) = min_age_days
                && !force
                && let Some(age_days) = version_age_days(package, version_str, now)
                && age_days < min_days as i64
            {
                match &latest_rejected {
                    Some((latest, _, _)) if ver <= *latest => {}
                    _ => latest_rejected = Some((ver, version_str.clone(), age_days)),
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
            && let Some((_, ver_str, age_days)) = latest_rejected
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
            reason: format!(
                "no published version matches this range. {}",
                available_versions_hint(package)
            ),
        })
    }
}

/// A short hint listing the newest available versions, so a failed
/// resolve tells the user what they *could* pick instead of a bare
/// "not found".
fn available_versions_hint(package: &RegistryPackage) -> String {
    let mut versions: Vec<Version> = package
        .versions
        .keys()
        .filter_map(|version| parse_version(version).ok())
        .collect();

    if versions.is_empty() {
        return "The package has no published versions.".to_string();
    }

    versions.sort();
    versions.reverse();
    let shown: Vec<String> = versions.iter().take(5).map(|v| v.to_string()).collect();
    let latest_tag = package
        .dist_tags
        .get("latest")
        .map(|latest| format!(" (latest: {latest})"))
        .unwrap_or_default();

    format!(
        "Available versions include: {}{}{}.",
        shown.join(", "),
        if versions.len() > shown.len() {
            format!(", … ({} total)", versions.len())
        } else {
            String::new()
        },
        latest_tag
    )
}

pub(crate) fn version_age_days(
    package: &RegistryPackage,
    version: &str,
    now: OffsetDateTime,
) -> Option<i64> {
    let time_str = package.time.get(version)?;
    let published = OffsetDateTime::parse(time_str, &Rfc3339).ok()?;
    let age = now - published;
    Some(age.whole_days())
}
