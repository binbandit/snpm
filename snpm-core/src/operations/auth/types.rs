#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AuthType {
    #[default]
    Web,
    Legacy,
}

#[derive(Debug, Clone)]
pub struct AuthResult {
    pub token: String,
    pub username: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub struct Credentials {
    pub username: Option<String>,
    pub password: Option<String>,
}

pub type OpenerFn = Box<dyn Fn(&str) -> std::result::Result<(), String> + Send + Sync>;
