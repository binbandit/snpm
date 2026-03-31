use crate::resolve::{PackageId, ResolutionGraph, ResolvedPackage};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallScenario {
    Hot,
    WarmLinkOnly,
    WarmPartialCache,
    Cold,
}

pub struct CacheCheckResult {
    pub cached: BTreeMap<PackageId, PathBuf>,
    pub missing: Vec<ResolvedPackage>,
}

#[derive(Debug, Clone)]
pub struct InstallOptions {
    pub requested: Vec<String>,
    pub dev: bool,
    pub include_dev: bool,
    pub frozen_lockfile: bool,
    pub force: bool,
    pub silent_summary: bool,
}

#[derive(Debug, Clone)]
pub struct InstallResult {
    pub package_count: usize,
    pub elapsed_seconds: f32,
}

#[derive(Debug)]
pub struct OutdatedEntry {
    pub name: String,
    pub current: Option<String>,
    pub wanted: String,
}

#[derive(Debug, Clone)]
pub struct ParsedSpec {
    pub name: String,
    pub range: String,
    pub protocol: Option<String>,
}

pub struct ScenarioResult {
    pub scenario: InstallScenario,
    pub cache_check: Option<CacheCheckResult>,
    pub graph: Option<ResolutionGraph>,
    pub integrity_state: Option<IntegrityState>,
}

impl ScenarioResult {
    pub fn cold() -> Self {
        Self {
            scenario: InstallScenario::Cold,
            cache_check: None,
            graph: None,
            integrity_state: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct IntegrityState {
    pub lockfile_hash: String,
    pub patch_hash: String,
}
