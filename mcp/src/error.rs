#[derive(Debug)]
pub struct Error {
    pub message: String,
    pub code: i32,
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self {
            message: format!("{error}"),
            code: 500,
        }
    }
}
