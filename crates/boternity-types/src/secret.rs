use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::bot::BotId;

use std::fmt;

/// A secret key identifier (e.g., "ANTHROPIC_API_KEY").
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SecretKey(pub String);

impl SecretKey {
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }
}

impl fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SecretKey(\"{}\")", self.0)
    }
}

impl fmt::Display for SecretKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Metadata about a stored secret (the value itself is never in this struct).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretEntry {
    /// The key name (e.g., "ANTHROPIC_API_KEY").
    pub key: SecretKey,
    /// Where this secret is stored.
    pub provider: SecretProvider,
    /// Whether this is global or bot-scoped.
    pub scope: SecretScope,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Storage backend for a secret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SecretProvider {
    /// Encrypted vault file (~/.boternity/vault.enc).
    Vault,
    /// OS keychain (macOS Keychain / Linux Secret Service).
    Keychain,
    /// Environment variable.
    Environment,
}

impl fmt::Display for SecretProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SecretProvider::Vault => write!(f, "vault"),
            SecretProvider::Keychain => write!(f, "keychain"),
            SecretProvider::Environment => write!(f, "environment"),
        }
    }
}

/// Scope determines whether a secret is globally available or bound to a specific bot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SecretScope {
    /// Available to all bots.
    Global,
    /// Available only to the specified bot.
    Bot(BotId),
}

impl fmt::Display for SecretScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SecretScope::Global => write!(f, "global"),
            SecretScope::Bot(id) => write!(f, "bot:{id}"),
        }
    }
}

/// A wrapper that redacts secret values in Debug and Display output.
///
/// Use this to wrap any `String` that might contain sensitive data.
/// The actual value is accessible via `.expose()`.
#[derive(Clone, Serialize, Deserialize)]
pub struct Redacted(String);

impl Redacted {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Access the underlying secret value.
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// Show masked representation: last 4 chars visible.
    pub fn masked(&self) -> String {
        if self.0.len() <= 4 {
            "****".to_string()
        } else {
            format!("****{}", &self.0[self.0.len() - 4..])
        }
    }
}

impl fmt::Debug for Redacted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Redacted(\"***\")")
    }
}

impl fmt::Display for Redacted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "***")
    }
}

/// A generic wrapper that redacts any value in Debug and Display output.
///
/// Use this to wrap any type that might contain sensitive data.
/// Unlike [`Redacted`] (which only wraps `String`), `Secret<T>` works with any type.
///
/// # Examples
///
/// ```
/// use boternity_types::secret::Secret;
///
/// let api_key = Secret::new("sk-secret-key".to_string());
/// assert_eq!(format!("{:?}", api_key), "***REDACTED***");
/// assert_eq!(format!("{}", api_key), "***REDACTED***");
/// assert_eq!(api_key.expose(), "sk-secret-key");
/// ```
#[derive(Clone, Serialize, Deserialize)]
pub struct Secret<T>(T);

impl<T> Secret<T> {
    /// Wrap a value in a Secret container.
    pub fn new(value: T) -> Self {
        Self(value)
    }

    /// Access the underlying secret value.
    ///
    /// This is an explicit operation to prevent accidental logging.
    /// Only call this when you genuinely need the raw value.
    pub fn expose(&self) -> &T {
        &self.0
    }

    /// Consume the wrapper and return the inner value.
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> fmt::Debug for Secret<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "***REDACTED***")
    }
}

impl<T> fmt::Display for Secret<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "***REDACTED***")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redacted_debug_hides_value() {
        let secret = Redacted::new("sk-abc123xyz");
        let debug = format!("{:?}", secret);
        assert!(!debug.contains("abc123xyz"));
        assert!(debug.contains("***"));
    }

    #[test]
    fn test_redacted_display_hides_value() {
        let secret = Redacted::new("sk-abc123xyz");
        let display = format!("{}", secret);
        assert!(!display.contains("abc123xyz"));
    }

    #[test]
    fn test_redacted_expose() {
        let secret = Redacted::new("sk-abc123xyz");
        assert_eq!(secret.expose(), "sk-abc123xyz");
    }

    #[test]
    fn test_redacted_masked() {
        let secret = Redacted::new("sk-abc123xyz");
        assert_eq!(secret.masked(), "****3xyz");
    }

    #[test]
    fn test_redacted_masked_short() {
        let secret = Redacted::new("ab");
        assert_eq!(secret.masked(), "****");
    }

    #[test]
    fn test_secret_scope_display() {
        assert_eq!(SecretScope::Global.to_string(), "global");
        let bot_id = BotId::new();
        let scope = SecretScope::Bot(bot_id.clone());
        assert!(scope.to_string().starts_with("bot:"));
    }

    #[test]
    fn test_secret_debug_redacted() {
        let secret = Secret::new("sk-super-secret-key".to_string());
        let debug = format!("{:?}", secret);
        assert_eq!(debug, "***REDACTED***");
        assert!(!debug.contains("sk-super-secret-key"));
    }

    #[test]
    fn test_secret_display_redacted() {
        let secret = Secret::new("sk-super-secret-key".to_string());
        let display = format!("{}", secret);
        assert_eq!(display, "***REDACTED***");
        assert!(!display.contains("sk-super-secret-key"));
    }

    #[test]
    fn test_secret_expose() {
        let secret = Secret::new("my-api-key".to_string());
        assert_eq!(secret.expose(), "my-api-key");
    }

    #[test]
    fn test_secret_into_inner() {
        let secret = Secret::new(42u64);
        assert_eq!(secret.into_inner(), 42);
    }

    #[test]
    fn test_secret_generic_with_i32() {
        let secret = Secret::new(12345i32);
        assert_eq!(format!("{:?}", secret), "***REDACTED***");
        assert_eq!(*secret.expose(), 12345);
    }
}
