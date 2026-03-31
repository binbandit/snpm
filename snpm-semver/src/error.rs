use std::error::Error as StdError;
use std::fmt;

#[derive(Debug, Clone)]
pub struct Error {
    input: String,
    message: String,
}

impl Error {
    pub fn new(input: String, message: String) -> Self {
        Self { input, message }
    }

    pub fn input(&self) -> &str {
        &self.input
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.message, self.input)
    }
}

impl StdError for Error {}
