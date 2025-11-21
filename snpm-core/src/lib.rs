pub mod config;
pub mod error;
pub mod operations;
pub mod project;

pub use config::SnpmConfig;
pub use error::SnpmError;
pub use project::Project;

pub type Result<T> = std::result::Result<T, SnpmError>;
