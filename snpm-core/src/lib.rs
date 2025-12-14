pub mod cache;
pub mod config;
pub mod console;
pub mod error;
pub mod lifecycle;
pub mod linker;
pub mod lockfile;
pub mod operations;
pub mod platform;
pub mod project;
pub mod protocols;
pub mod registry;
pub mod resolve;
pub mod store;
pub mod version;
pub mod workspace;

pub use config::{HoistingMode, LinkBackend, SnpmConfig};
pub use error::SnpmError;
pub use project::Project;
pub use workspace::Workspace;

pub type Result<T> = std::result::Result<T, SnpmError>;
