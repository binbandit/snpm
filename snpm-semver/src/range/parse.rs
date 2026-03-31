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
    let tokens: Vec<&str> = part.split_whitespace().collect();

    if tokens.len() <= 1 {
        return part.to_string();
    }

    if tokens.len() == 3 && tokens[1] == "-" {
        return expand_hyphen_range(tokens[0], tokens[2]);
    }

    let mut result = String::new();
    for (index, token) in tokens.iter().enumerate() {
        if index > 0 {
            let previous = tokens[index - 1];
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
        format!("={input}")
    } else {
        input.to_string()
    }
}
