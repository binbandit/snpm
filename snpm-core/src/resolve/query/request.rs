use crate::registry::RegistryProtocol;

#[derive(Clone, Debug)]
pub struct DepRequest {
    pub source: String,
    pub range: String,
    pub protocol: RegistryProtocol,
}
