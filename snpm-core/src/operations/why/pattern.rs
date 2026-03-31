pub(super) fn matches_pattern(value: &str, pattern: &str) -> bool {
    if pattern == "*" {
        return true;
    }

    if !pattern.contains('*') {
        return value == pattern;
    }

    let starts_anchored = !pattern.starts_with('*');
    let ends_anchored = !pattern.ends_with('*');
    let parts: Vec<&str> = pattern.split('*').filter(|part| !part.is_empty()).collect();

    if parts.is_empty() {
        return true;
    }

    let mut cursor = 0usize;

    for (idx, part) in parts.iter().enumerate() {
        if idx == 0 && starts_anchored {
            if !value[cursor..].starts_with(part) {
                return false;
            }
            cursor += part.len();
            continue;
        }

        let Some(found) = value[cursor..].find(part) else {
            return false;
        };

        cursor += found + part.len();
    }

    if ends_anchored {
        if let Some(last) = parts.last() {
            value.ends_with(last)
        } else {
            true
        }
    } else {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::matches_pattern;

    #[test]
    fn wildcard_matching_supports_star() {
        assert!(matches_pattern("@types/react", "@types/*"));
        assert!(matches_pattern("left-pad", "*pad"));
        assert!(matches_pattern("left-pad", "left*"));
        assert!(!matches_pattern("react", "@types/*"));
    }
}
