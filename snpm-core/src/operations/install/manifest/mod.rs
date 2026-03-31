mod apply;
mod parsing;
mod protocol;
mod root_specs;
mod types;
mod write;

pub use apply::{apply_specs, resolve_catalog_spec};
pub use parsing::{parse_requested_spec, parse_requested_with_protocol, parse_spec};
pub use protocol::{detect_manifest_protocol, is_special_protocol_spec};
pub use root_specs::{build_project_manifest_root, build_project_root_specs};
pub use types::RootSpecSet;
pub use write::write_manifest;
