use std::fmt;

pub type StudioResult<T> = Result<T, StudioError>;

#[derive(Debug)]
pub struct StudioError(pub String);

impl StudioError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for StudioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for StudioError {}

impl From<std::io::Error> for StudioError {
    fn from(value: std::io::Error) -> Self {
        Self(value.to_string())
    }
}

impl From<serde_json::Error> for StudioError {
    fn from(value: serde_json::Error) -> Self {
        Self(value.to_string())
    }
}

impl From<String> for StudioError {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for StudioError {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}
