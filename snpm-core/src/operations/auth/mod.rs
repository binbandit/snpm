mod persistence;
mod remote;
mod types;

pub use persistence::{login, logout, save_credentials};
pub use remote::login_with_fallback;
pub use types::{AuthResult, AuthType, Credentials, OpenerFn};
