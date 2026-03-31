#[derive(Debug, Default)]
pub struct CleanSummary {
    pub packages_count: usize,
    pub packages_size: u64,
    pub metadata_count: usize,
    pub metadata_size: u64,
    pub global_count: usize,
    pub global_size: u64,
}

impl CleanSummary {
    pub fn total_size(&self) -> u64 {
        self.packages_size + self.metadata_size + self.global_size
    }

    pub fn total_count(&self) -> usize {
        self.packages_count + self.metadata_count + self.global_count
    }

    pub fn is_empty(&self) -> bool {
        self.total_count() == 0
    }
}

#[derive(Debug, Clone)]
pub struct CleanOptions {
    pub packages: bool,
    pub metadata: bool,
    pub global: bool,
}

impl Default for CleanOptions {
    fn default() -> Self {
        Self {
            packages: true,
            metadata: true,
            global: false,
        }
    }
}

impl CleanOptions {
    pub fn all() -> Self {
        Self {
            packages: true,
            metadata: true,
            global: true,
        }
    }
}

pub fn format_bytes(bytes: u64) -> String {
    const KILOBYTE: u64 = 1024;
    const MEGABYTE: u64 = KILOBYTE * 1024;
    const GIGABYTE: u64 = MEGABYTE * 1024;

    if bytes >= GIGABYTE {
        format!("{:.2} GB", bytes as f64 / GIGABYTE as f64)
    } else if bytes >= MEGABYTE {
        format!("{:.2} MB", bytes as f64 / MEGABYTE as f64)
    } else if bytes >= KILOBYTE {
        format!("{:.2} KB", bytes as f64 / KILOBYTE as f64)
    } else {
        format!("{} bytes", bytes)
    }
}
