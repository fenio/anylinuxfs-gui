use thiserror::Error;

/// Application-specific errors with proper context
#[derive(Debug, Error)]
pub enum AppError {
    #[error("CLI not found: {0}")]
    CliNotFound(String),

    #[error("CLI execution failed: {0}")]
    CliError(String),

    #[error("Mount failed: {0}")]
    MountFailed(String),

    #[error("Unmount failed: {0}")]
    UnmountFailed(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("TOML parse error: {0}")]
    TomlParseError(#[from] toml::de::Error),

    #[error("TOML serialize error: {0}")]
    TomlSerializeError(#[from] toml::ser::Error),

    #[error("Task error: {0}")]
    TaskError(String),

    #[error("Operation timed out after {0} seconds")]
    Timeout(u64),

    #[error("Shell error: {0}")]
    ShellError(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("{0}")]
    Other(String),
}

// Convert AppError to String for Tauri command returns
impl From<AppError> for String {
    fn from(err: AppError) -> String {
        err.to_string()
    }
}

// Helper to convert tokio JoinError
impl From<tokio::task::JoinError> for AppError {
    fn from(err: tokio::task::JoinError) -> Self {
        AppError::TaskError(err.to_string())
    }
}

// Result type alias for convenience
pub type AppResult<T> = Result<T, AppError>;
