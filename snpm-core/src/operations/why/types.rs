use serde::Serialize;

#[derive(Debug, Clone, Copy)]
pub struct WhyOptions {
    pub depth: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct WhyResult {
    pub matches: Vec<WhyPackageMatch>,
}

#[derive(Debug, Serialize)]
pub struct WhyPackageMatch {
    pub name: String,
    pub version: String,
    pub paths: Vec<WhyPath>,
}

#[derive(Debug, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct WhyPath {
    pub hops: Vec<WhyHop>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum WhyHop {
    Package {
        name: String,
        version: String,
        via: String,
    },
    Root {
        name: String,
        requested: String,
    },
}
