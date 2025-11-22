pub mod config;
pub mod error;
pub mod lifecycle;
pub mod linker;
pub mod lockfile;
pub mod operations;
pub mod project;
pub mod registry;
pub mod resolve;
pub mod store;
pub mod workspace;

pub use config::SnpmConfig;
pub use error::SnpmError;
pub use project::Project;
pub use workspace::Workspace;

pub type Result<T> = std::result::Result<T, SnpmError>;
