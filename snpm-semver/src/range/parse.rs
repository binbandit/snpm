use super::RangeSet;
use crate::Error;
use semver::VersionReq;

pub(super) fn parse_internal(original: &str) -> Result<RangeSet, Error> {
    let mut source = original.trim();

    if source.is_empty() || source == "latest" {
        source = "*";
    }

    if (source.starts_with("npm:") || source.starts_with("jsr:"))
        && let Some(colon) = source.find(':')
    {
        source = extract_version_range(&source[colon + 1..]);
    }

    let mut ranges = Vec::new();

    for part in source.split("||") {
        let normalized = normalize_part(part);
        if normalized.is_empty() {
            continue;
        }

        let adjusted = adjust_node_default(&normalized);
        let request = VersionReq::parse(&adjusted)
            .map_err(|error| Error::new(original.to_string(), error.to_string()))?;
        ranges.push(request);
    }

    if ranges.is_empty() {
        let request = VersionReq::parse("*")
            .map_err(|error| Error::new(original.to_string(), error.to_string()))?;
        ranges.push(request);
    }

    Ok(RangeSet {
        original: original.to_string(),
        ranges,
    })
}

pub(super) fn normalize_and_part(part: &str) -> String {
    let tokens: Vec<String> = part.split_whitespace().map(strip_v_prefix).collect();

    if tokens.is_empty() {
        return part.to_string();
    }

    if tokens.len() == 1 {
        return tokens.into_iter().next().unwrap();
    }

    if tokens.len() == 3 && tokens[1] == "-" {
        return expand_hyphen_range(&tokens[0], &tokens[2]);
    }

    let mut result = String::new();
    for (index, token) in tokens.iter().enumerate() {
        if index > 0 {
            let previous = tokens[index - 1].as_str();
            if matches!(previous, "=" | ">" | ">=" | "<" | "<=" | "~" | "^") {
                result.push(' ');
            } else {
                result.push_str(", ");
            }
        }

        result.push_str(token);
    }

    result
}

/// npm's loose parsing accepts a `v` prefix on versions inside ranges
/// (`v1.2.3`, `>=v1.2.3`). The `semver` crate rejects it, so strip it
/// from each token when it directly precedes a digit.
fn strip_v_prefix(token: &str) -> String {
    let operator_len = token
        .find(|character: char| !matches!(character, '<' | '>' | '=' | '~' | '^'))
        .unwrap_or(token.len());
    let (operator, rest) = token.split_at(operator_len);

    let mut rest_chars = rest.chars();
    if matches!(rest_chars.next(), Some('v') | Some('V'))
        && rest_chars.next().is_some_and(|c| c.is_ascii_digit())
    {
        format!("{operator}{}", &rest[1..])
    } else {
        token.to_string()
    }
}

pub(super) fn is_plain_exact_version(input: &str) -> bool {
    if input.is_empty() {
        return false;
    }

    if input
        .chars()
        .any(|character| matches!(character, '<' | '>' | '=' | '^' | '~' | ' ' | ','))
    {
        return false;
    }

    if input.contains('*') {
        return false;
    }

    let version_part = input.split(['-', '+']).next().unwrap_or(input);
    for segment in version_part.split('.') {
        if segment.eq_ignore_ascii_case("x") {
            return false;
        }
    }

    input.chars().filter(|&character| character == '.').count() >= 2
}

fn extract_version_range(protocol_spec: &str) -> &str {
    let search_from = if protocol_spec.starts_with('@') {
        protocol_spec.find('/').map(|index| index + 1).unwrap_or(1)
    } else {
        0
    };

    if let Some(at) = protocol_spec[search_from..].rfind('@') {
        let version = protocol_spec[search_from + at + 1..].trim();
        if version.is_empty() { "*" } else { version }
    } else {
        "*"
    }
}

fn normalize_part(part: &str) -> String {
    let mut part = part.trim();

    if part.starts_with('@') && part.len() > 1 {
        let second = part.as_bytes()[1] as char;
        if second.is_ascii_digit() || matches!(second, '^' | '~' | '>' | '<' | '=') {
            part = &part[1..];
        }
    }

    if part.is_empty() || part == "latest" {
        return "*".to_string();
    }

    normalize_and_part(part)
}

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
        _ => format!(">={}, <={}", lower_full, upper),
    }
}

fn adjust_node_default(input: &str) -> String {
    if is_plain_exact_version(input) {
        return format!("={input}");
    }

    if let Some(expanded) = expand_bare_major_minor(input) {
        return expanded;
    }

    input.to_string()
}

/// npm treats a bare `X.Y` as the partial range `X.Y.x` (>=X.Y.0 <X.(Y+1).0),
/// but the `semver` crate would give it cargo caret semantics
/// (>=X.Y.0 <(X+1).0.0). Expand it explicitly so npm semantics win.
fn expand_bare_major_minor(input: &str) -> Option<String> {
    let (major, minor) = input.split_once('.')?;
    if major.is_empty()
        || minor.is_empty()
        || !major.bytes().all(|byte| byte.is_ascii_digit())
        || !minor.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }

    let next_minor = minor.parse::<u64>().ok()?.checked_add(1)?;
    Some(format!(">={major}.{minor}.0, <{major}.{next_minor}.0"))
}
