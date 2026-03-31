#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OfflineMode {
    #[default]
    Online,
    PreferOffline,
    Offline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthScheme {
    Bearer,
    Basic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkBackend {
    Auto,
    Reflink,
    Hardlink,
    Symlink,
    Copy,
}

impl LinkBackend {
    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "auto" | "default" => Some(LinkBackend::Auto),
            "reflink" | "reflinks" | "cow" | "clone" => Some(LinkBackend::Reflink),
            "hardlink" | "hardlinks" | "hard" => Some(LinkBackend::Hardlink),
            "symlink" | "symlinks" | "symbolic" | "sym" => Some(LinkBackend::Symlink),
            "copy" | "copies" => Some(LinkBackend::Copy),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoistingMode {
    None,
    SingleVersion,
    All,
}

impl HoistingMode {
    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "none" | "off" | "false" | "disabled" => Some(HoistingMode::None),
            "single" | "single-version" | "safe" => Some(HoistingMode::SingleVersion),
            "root" | "all" | "true" | "on" | "enabled" => Some(HoistingMode::All),
            _ => None,
        }
    }
}
