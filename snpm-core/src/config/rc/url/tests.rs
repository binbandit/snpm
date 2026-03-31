use super::*;

#[test]
fn normalize_registry_url_strips_trailing_slash() {
    assert_eq!(
        normalize_registry_url("https://registry.npmjs.org/"),
        "https://registry.npmjs.org"
    );
}

#[test]
fn normalize_registry_url_lowercases_host() {
    assert_eq!(
        normalize_registry_url("https://Registry.NPMjs.ORG"),
        "https://registry.npmjs.org"
    );
}

#[test]
fn normalize_registry_url_strips_default_https_port() {
    assert_eq!(
        normalize_registry_url("https://registry.npmjs.org:443/"),
        "https://registry.npmjs.org"
    );
}

#[test]
fn normalize_registry_url_strips_default_http_port() {
    assert_eq!(
        normalize_registry_url("http://localhost:80/"),
        "http://localhost"
    );
}

#[test]
fn normalize_registry_url_keeps_custom_port() {
    assert_eq!(
        normalize_registry_url("http://localhost:4873"),
        "http://localhost:4873"
    );
}

#[test]
fn normalize_registry_url_adds_https_to_double_slash() {
    assert_eq!(
        normalize_registry_url("//registry.npmjs.org/"),
        "https://registry.npmjs.org"
    );
}

#[test]
fn normalize_registry_url_preserves_path() {
    assert_eq!(
        normalize_registry_url("https://npm.pkg.github.com/owner"),
        "https://npm.pkg.github.com/owner"
    );
}

#[test]
fn host_from_url_basic() {
    assert_eq!(
        host_from_url("https://registry.npmjs.org/foo"),
        Some("registry.npmjs.org".to_string())
    );
}

#[test]
fn host_from_url_strips_default_port() {
    assert_eq!(
        host_from_url("https://registry.npmjs.org:443/foo"),
        Some("registry.npmjs.org".to_string())
    );
}

#[test]
fn host_from_url_keeps_custom_port() {
    assert_eq!(
        host_from_url("http://localhost:4873"),
        Some("localhost:4873".to_string())
    );
}

#[test]
fn host_from_url_empty_returns_none() {
    assert_eq!(host_from_url(""), None);
}

#[test]
fn host_from_url_lowercases() {
    assert_eq!(
        host_from_url("https://Registry.NPMjs.ORG/"),
        Some("registry.npmjs.org".to_string())
    );
}
