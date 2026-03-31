use crate::registry::RegistryProtocol;

pub(in crate::resolve) fn protocol_from_range(range: &str) -> RegistryProtocol {
    if range.starts_with("file:") {
        RegistryProtocol::file()
    } else if range.starts_with("git:") || range.starts_with("git+") {
        RegistryProtocol::git()
    } else if range.starts_with("jsr:") {
        RegistryProtocol::jsr()
    } else {
        RegistryProtocol::npm()
    }
}
