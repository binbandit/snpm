#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct SwitchOptions {
    pub(crate) ignore_package_manager: bool,
    pub(crate) version_override: Option<String>,
}
