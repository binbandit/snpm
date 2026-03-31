pub(super) fn filter_session_marker(content: &str) -> String {
    let mut result = String::new();
    let mut skip_until_next_diff = false;

    for line in content.lines() {
        if line.starts_with("diff ") {
            skip_until_next_diff = line.contains(super::super::SESSION_MARKER);
        }

        if !skip_until_next_diff {
            result.push_str(line);
            result.push('\n');
        }
    }

    result
}
