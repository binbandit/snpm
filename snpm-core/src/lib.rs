pub mod config;
pub mod error;
pub mod project;
pub mod operations;

pub use config::SnpmConfig;
pub use error::SnpmError;
pub use project::Project;

pub type Result<T> = std::result::Result<T, SnpmError>;
