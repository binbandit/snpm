pub mod config;
pub mod error;
pub mod linker;
pub mod lockfile;
pub mod operations;
pub mod project;
pub mod registry;
pub mod resolve;
pub mod store;

pub use config::SnpmConfig;
pub use error::SnpmError;
pub use project::Project;

pub type Result<T> = std::result::Result<T, SnpmError>;
