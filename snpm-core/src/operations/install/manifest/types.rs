use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct RootSpecSet {
    pub required: BTreeMap<String, String>,
    pub optional: BTreeMap<String, String>,
}
