use serde::{Deserialize, Serialize};

use super::severity::Severity;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VulnerabilityCounts {
    #[serde(default)]
    pub info: u64,
    #[serde(default)]
    pub low: u64,
    #[serde(default)]
    pub moderate: u64,
    #[serde(default)]
    pub high: u64,
    #[serde(default)]
    pub critical: u64,
}

impl VulnerabilityCounts {
    pub fn total(&self) -> u64 {
        self.info + self.low + self.moderate + self.high + self.critical
    }

    pub fn above_threshold(&self, threshold: Severity) -> u64 {
        match threshold {
            Severity::Info => self.total(),
            Severity::Low => self.low + self.moderate + self.high + self.critical,
            Severity::Moderate => self.moderate + self.high + self.critical,
            Severity::High => self.high + self.critical,
            Severity::Critical => self.critical,
        }
    }

    pub fn merge(&mut self, other: &VulnerabilityCounts) {
        self.info += other.info;
        self.low += other.low;
        self.moderate += other.moderate;
        self.high += other.high;
        self.critical += other.critical;
    }
}
