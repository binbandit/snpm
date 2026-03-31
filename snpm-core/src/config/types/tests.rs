use super::{HoistingMode, LinkBackend};

#[test]
fn link_backend_parse_auto() {
    assert_eq!(LinkBackend::parse("auto"), Some(LinkBackend::Auto));
    assert_eq!(LinkBackend::parse("default"), Some(LinkBackend::Auto));
}

#[test]
fn link_backend_parse_reflink() {
    assert_eq!(LinkBackend::parse("reflink"), Some(LinkBackend::Reflink));
    assert_eq!(LinkBackend::parse("cow"), Some(LinkBackend::Reflink));
    assert_eq!(LinkBackend::parse("clone"), Some(LinkBackend::Reflink));
}

#[test]
fn link_backend_parse_hardlink() {
    assert_eq!(LinkBackend::parse("hardlink"), Some(LinkBackend::Hardlink));
    assert_eq!(LinkBackend::parse("hard"), Some(LinkBackend::Hardlink));
}

#[test]
fn link_backend_parse_symlink() {
    assert_eq!(LinkBackend::parse("symlink"), Some(LinkBackend::Symlink));
    assert_eq!(LinkBackend::parse("sym"), Some(LinkBackend::Symlink));
}

#[test]
fn link_backend_parse_copy() {
    assert_eq!(LinkBackend::parse("copy"), Some(LinkBackend::Copy));
    assert_eq!(LinkBackend::parse("copies"), Some(LinkBackend::Copy));
}

#[test]
fn link_backend_parse_unknown() {
    assert_eq!(LinkBackend::parse("unknown"), None);
}

#[test]
fn link_backend_parse_case_insensitive() {
    assert_eq!(LinkBackend::parse("AUTO"), Some(LinkBackend::Auto));
    assert_eq!(LinkBackend::parse("Hardlink"), Some(LinkBackend::Hardlink));
}

#[test]
fn hoisting_mode_parse_none() {
    assert_eq!(HoistingMode::parse("none"), Some(HoistingMode::None));
    assert_eq!(HoistingMode::parse("off"), Some(HoistingMode::None));
    assert_eq!(HoistingMode::parse("false"), Some(HoistingMode::None));
    assert_eq!(HoistingMode::parse("disabled"), Some(HoistingMode::None));
}

#[test]
fn hoisting_mode_parse_single() {
    assert_eq!(
        HoistingMode::parse("single"),
        Some(HoistingMode::SingleVersion)
    );
    assert_eq!(
        HoistingMode::parse("single-version"),
        Some(HoistingMode::SingleVersion)
    );
    assert_eq!(
        HoistingMode::parse("safe"),
        Some(HoistingMode::SingleVersion)
    );
}

#[test]
fn hoisting_mode_parse_all() {
    assert_eq!(HoistingMode::parse("root"), Some(HoistingMode::All));
    assert_eq!(HoistingMode::parse("all"), Some(HoistingMode::All));
    assert_eq!(HoistingMode::parse("true"), Some(HoistingMode::All));
}

#[test]
fn hoisting_mode_parse_unknown() {
    assert_eq!(HoistingMode::parse("unknown"), None);
}
