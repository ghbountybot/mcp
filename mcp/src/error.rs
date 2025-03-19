use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub struct Error {
    pub message: String,
    pub code: i32,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Error {}: {}", self.code, self.message)
    }
}

impl std::error::Error for Error {
    // No additional methods needed for this basic implementation
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self {
            message: format!("{error}"),
            code: 500,
        }
    }
}

impl From<eyre::Error> for Error {
    fn from(error: eyre::Report) -> Self {
        Self {
            message: format!("{error}"),
            code: 500,
        }
    }
}
