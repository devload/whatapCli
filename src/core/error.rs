use thiserror::Error;

#[derive(Debug, Error)]
pub enum CliError {
    #[error("Authentication required. Run 'whatap login' first.")]
    NotAuthenticated,

    #[error("Session expired. Run 'whatap login' to re-authenticate.")]
    SessionExpired,

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("API error ({status}): {message}")]
    Api { status: u16, message: String },

    #[error("Upload failed: {0}")]
    Upload(String),

    #[error("Invalid input: {0}")]
    Input(String),

    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("{0}")]
    Other(String),
}

impl CliError {
    pub fn exit_code(&self) -> i32 {
        match self {
            CliError::NotAuthenticated | CliError::SessionExpired => 2,
            CliError::Config(_) => 3,
            CliError::Api { .. } => 4,
            CliError::Upload(_) => 5,
            CliError::Input(_) | CliError::FileNotFound(_) => 6,
            CliError::Other(_) => 1,
        }
    }
}
