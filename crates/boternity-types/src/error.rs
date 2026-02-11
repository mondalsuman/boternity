use thiserror::Error;

/// Errors related to bot operations.
#[derive(Debug, Error)]
pub enum BotError {
    #[error("bot not found")]
    NotFound,

    #[error("slug '{0}' already exists")]
    SlugConflict(String),

    #[error("invalid bot status: '{0}'")]
    InvalidStatus(String),

    #[error("invalid bot name: {0}")]
    InvalidName(String),

    #[error("storage error: {0}")]
    StorageError(String),

    #[error("filesystem error: {0}")]
    FileSystemError(String),

    #[error("soul integrity violation: expected hash '{expected}', got '{actual}'")]
    SoulIntegrityViolation { expected: String, actual: String },
}

/// Errors related to soul operations.
#[derive(Debug, Error)]
pub enum SoulError {
    #[error("soul not found")]
    NotFound,

    #[error("soul hash mismatch: expected '{expected}', got '{actual}'")]
    HashMismatch { expected: String, actual: String },

    #[error("soul integrity violation")]
    IntegrityViolation,

    #[error("storage error: {0}")]
    StorageError(String),

    #[error("filesystem error: {0}")]
    FileSystemError(String),

    #[error("invalid soul content: {0}")]
    InvalidContent(String),
}

/// Errors related to secret operations.
#[derive(Debug, Error)]
pub enum SecretError {
    #[error("secret not found")]
    NotFound,

    #[error("secret provider unavailable")]
    ProviderUnavailable,

    #[error("encryption error")]
    EncryptionError,

    #[error("storage error: {0}")]
    StorageError(String),
}

/// Errors from repository operations (used by trait definitions in boternity-core).
#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("database connection error")]
    Connection,

    #[error("query error: {0}")]
    Query(String),

    #[error("entity not found")]
    NotFound,

    #[error("conflict: {0}")]
    Conflict(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bot_error_display() {
        let err = BotError::SlugConflict("luna".to_string());
        assert_eq!(err.to_string(), "slug 'luna' already exists");
    }

    #[test]
    fn test_soul_error_display() {
        let err = SoulError::HashMismatch {
            expected: "abc".to_string(),
            actual: "def".to_string(),
        };
        assert!(err.to_string().contains("abc"));
        assert!(err.to_string().contains("def"));
    }

    #[test]
    fn test_repository_error_display() {
        let err = RepositoryError::Query("syntax error".to_string());
        assert_eq!(err.to_string(), "query error: syntax error");
    }
}
