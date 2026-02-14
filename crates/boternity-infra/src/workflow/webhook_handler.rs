//! Webhook trigger handler with HMAC-SHA256 and bearer token authentication.
//!
//! Provides:
//! - `verify_hmac_sha256()` -- constant-time HMAC-SHA256 signature verification
//! - `verify_bearer_token()` -- constant-time bearer token comparison
//! - `WebhookRegistry` -- DashMap-backed registry for path -> webhook config lookup

use std::sync::Arc;

use dashmap::DashMap;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use uuid::Uuid;

// Type alias for HMAC-SHA256
type HmacSha256 = Hmac<Sha256>;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur during webhook handling.
#[derive(Debug, thiserror::Error)]
pub enum WebhookError {
    /// HMAC signature verification failed.
    #[error("HMAC signature verification failed")]
    HmacVerificationFailed,

    /// Bearer token verification failed.
    #[error("bearer token verification failed")]
    BearerVerificationFailed,

    /// No webhook registered at the given path.
    #[error("no webhook registered at path: {0}")]
    PathNotFound(String),

    /// Invalid HMAC key.
    #[error("invalid HMAC key: {0}")]
    InvalidKey(String),

    /// Missing authentication header.
    #[error("missing authentication: {0}")]
    MissingAuth(String),
}

// ---------------------------------------------------------------------------
// HMAC-SHA256 verification
// ---------------------------------------------------------------------------

/// Verify an HMAC-SHA256 signature against a request body.
///
/// Uses constant-time comparison to prevent timing attacks.
///
/// # Arguments
/// - `secret`: The shared secret key
/// - `body`: The raw request body bytes
/// - `signature_hex`: The hex-encoded HMAC signature to verify
///
/// # Returns
/// `Ok(())` if the signature is valid, `Err(WebhookError::HmacVerificationFailed)` otherwise.
pub fn verify_hmac_sha256(secret: &[u8], body: &[u8], signature_hex: &str) -> Result<(), WebhookError> {
    // Decode the expected signature from hex
    let expected_bytes = hex_decode(signature_hex)
        .map_err(|_| WebhookError::HmacVerificationFailed)?;

    // Compute HMAC
    let mut mac = HmacSha256::new_from_slice(secret)
        .map_err(|e| WebhookError::InvalidKey(e.to_string()))?;
    mac.update(body);

    // Constant-time verification (via hmac crate's `verify_slice`)
    mac.verify_slice(&expected_bytes)
        .map_err(|_| WebhookError::HmacVerificationFailed)
}

/// Verify an HMAC-SHA256 signature with an optional `sha256=` prefix.
///
/// GitHub-style webhooks send signatures as `sha256=<hex>`. This function
/// handles both prefixed and plain hex signatures.
pub fn verify_hmac_sha256_with_prefix(
    secret: &[u8],
    body: &[u8],
    signature: &str,
) -> Result<(), WebhookError> {
    let hex_sig = signature
        .strip_prefix("sha256=")
        .unwrap_or(signature);
    verify_hmac_sha256(secret, body, hex_sig)
}

// ---------------------------------------------------------------------------
// Bearer token verification
// ---------------------------------------------------------------------------

/// Verify a bearer token using constant-time comparison.
///
/// The `provided` token is compared against the `expected` token using
/// byte-by-byte XOR to prevent timing attacks.
///
/// # Arguments
/// - `expected`: The known-good token value
/// - `provided`: The token from the request (may have "Bearer " prefix)
///
/// # Returns
/// `Ok(())` if tokens match, `Err(WebhookError::BearerVerificationFailed)` otherwise.
pub fn verify_bearer_token(expected: &str, provided: &str) -> Result<(), WebhookError> {
    let token = provided
        .strip_prefix("Bearer ")
        .unwrap_or(provided);

    if constant_time_eq(expected.as_bytes(), token.as_bytes()) {
        Ok(())
    } else {
        Err(WebhookError::BearerVerificationFailed)
    }
}

// ---------------------------------------------------------------------------
// WebhookConfig
// ---------------------------------------------------------------------------

/// Authentication method for a registered webhook.
#[derive(Debug, Clone)]
pub enum WebhookAuthMethod {
    /// HMAC-SHA256 with the given secret bytes.
    HmacSha256 { secret: Vec<u8> },
    /// Bearer token.
    BearerToken { token: String },
    /// No authentication required.
    None,
}

/// Configuration for a registered webhook endpoint.
#[derive(Debug, Clone)]
pub struct WebhookConfig {
    /// The workflow ID this webhook triggers.
    pub workflow_id: Uuid,
    /// Workflow name (for logging).
    pub workflow_name: String,
    /// Authentication method.
    pub auth: WebhookAuthMethod,
    /// Optional JEXL `when` clause to filter payloads.
    pub when_clause: Option<String>,
}

// ---------------------------------------------------------------------------
// WebhookRegistry
// ---------------------------------------------------------------------------

/// Thread-safe registry mapping webhook paths to configurations.
///
/// Uses `DashMap` for concurrent read/write access without locking the
/// entire registry. Paths are normalized (always start with `/`).
pub struct WebhookRegistry {
    /// Path -> webhook config mapping.
    routes: Arc<DashMap<String, WebhookConfig>>,
}

impl WebhookRegistry {
    /// Create a new empty webhook registry.
    pub fn new() -> Self {
        Self {
            routes: Arc::new(DashMap::new()),
        }
    }

    /// Register a webhook at the given path.
    ///
    /// If a webhook already exists at this path, it is replaced.
    pub fn register(
        &self,
        path: &str,
        config: WebhookConfig,
    ) {
        let normalized = normalize_path(path);
        tracing::info!(
            path = %normalized,
            workflow_id = %config.workflow_id,
            "registered webhook"
        );
        self.routes.insert(normalized, config);
    }

    /// Unregister a webhook at the given path.
    ///
    /// Returns the removed config if one was registered.
    pub fn unregister(&self, path: &str) -> Option<WebhookConfig> {
        let normalized = normalize_path(path);
        self.routes.remove(&normalized).map(|(_, v)| v)
    }

    /// Look up a webhook config by path.
    pub fn lookup(&self, path: &str) -> Result<WebhookConfig, WebhookError> {
        let normalized = normalize_path(path);
        self.routes
            .get(&normalized)
            .map(|r| r.value().clone())
            .ok_or_else(|| WebhookError::PathNotFound(normalized))
    }

    /// Get the number of registered webhooks.
    pub fn len(&self) -> usize {
        self.routes.len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.routes.is_empty()
    }

    /// List all registered paths.
    pub fn paths(&self) -> Vec<String> {
        self.routes.iter().map(|r| r.key().clone()).collect()
    }

    /// Verify authentication for an incoming webhook request.
    ///
    /// Looks up the webhook config for the given path, then verifies the
    /// request against the configured authentication method.
    pub fn verify_request(
        &self,
        path: &str,
        body: &[u8],
        signature_header: Option<&str>,
        auth_header: Option<&str>,
    ) -> Result<WebhookConfig, WebhookError> {
        let config = self.lookup(path)?;

        match &config.auth {
            WebhookAuthMethod::HmacSha256 { secret } => {
                let sig = signature_header
                    .ok_or_else(|| WebhookError::MissingAuth(
                        "X-Hub-Signature-256 header required".to_string(),
                    ))?;
                verify_hmac_sha256_with_prefix(secret, body, sig)?;
            }
            WebhookAuthMethod::BearerToken { token } => {
                let auth = auth_header
                    .ok_or_else(|| WebhookError::MissingAuth(
                        "Authorization header required".to_string(),
                    ))?;
                verify_bearer_token(token, auth)?;
            }
            WebhookAuthMethod::None => {
                // No authentication required
            }
        }

        Ok(config)
    }
}

impl Default for WebhookRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Normalize a webhook path: ensure it starts with `/` and has no trailing slash.
fn normalize_path(path: &str) -> String {
    let mut normalized = path.to_string();
    if !normalized.starts_with('/') {
        normalized = format!("/{normalized}");
    }
    // Remove trailing slash (unless root)
    if normalized.len() > 1 && normalized.ends_with('/') {
        normalized.pop();
    }
    normalized
}

/// Decode a hex string to bytes.
fn hex_decode(hex: &str) -> Result<Vec<u8>, ()> {
    if hex.len() % 2 != 0 {
        return Err(());
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| ()))
        .collect()
}

/// Encode bytes to a lowercase hex string.
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Constant-time byte comparison (XOR-based).
///
/// Returns true if and only if `a == b`. Time taken is independent of
/// how many bytes match (mitigates timing attacks).
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
}

/// Compute HMAC-SHA256 and return hex-encoded signature.
///
/// Useful for generating test vectors and webhook signatures.
pub fn compute_hmac_sha256_hex(secret: &[u8], body: &[u8]) -> Result<String, WebhookError> {
    let mut mac = HmacSha256::new_from_slice(secret)
        .map_err(|e| WebhookError::InvalidKey(e.to_string()))?;
    mac.update(body);
    let result = mac.finalize();
    Ok(hex_encode(&result.into_bytes()))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------
    // HMAC-SHA256 verification
    // -------------------------------------------------------------------

    #[test]
    fn test_verify_hmac_sha256_valid() {
        let secret = b"my-webhook-secret";
        let body = b"Hello, world!";

        // Compute the expected signature
        let sig = compute_hmac_sha256_hex(secret, body).unwrap();

        // Verify should succeed
        assert!(verify_hmac_sha256(secret, body, &sig).is_ok());
    }

    #[test]
    fn test_verify_hmac_sha256_invalid_signature() {
        let secret = b"my-webhook-secret";
        let body = b"Hello, world!";
        let wrong_sig = "deadbeefcafebabe0000000000000000000000000000000000000000000000aa";

        assert!(verify_hmac_sha256(secret, body, wrong_sig).is_err());
    }

    #[test]
    fn test_verify_hmac_sha256_wrong_body() {
        let secret = b"my-webhook-secret";
        let body = b"Hello, world!";
        let sig = compute_hmac_sha256_hex(secret, body).unwrap();

        // Verify with different body should fail
        assert!(verify_hmac_sha256(secret, b"Different body", &sig).is_err());
    }

    #[test]
    fn test_verify_hmac_sha256_wrong_secret() {
        let secret = b"my-webhook-secret";
        let body = b"Hello, world!";
        let sig = compute_hmac_sha256_hex(secret, body).unwrap();

        // Verify with different secret should fail
        assert!(verify_hmac_sha256(b"wrong-secret", body, &sig).is_err());
    }

    #[test]
    fn test_verify_hmac_sha256_with_prefix() {
        let secret = b"my-webhook-secret";
        let body = b"payload data";
        let sig = compute_hmac_sha256_hex(secret, body).unwrap();

        // With sha256= prefix (GitHub style)
        let prefixed = format!("sha256={sig}");
        assert!(verify_hmac_sha256_with_prefix(secret, body, &prefixed).is_ok());

        // Without prefix
        assert!(verify_hmac_sha256_with_prefix(secret, body, &sig).is_ok());
    }

    #[test]
    fn test_verify_hmac_sha256_invalid_hex() {
        let secret = b"my-webhook-secret";
        let body = b"Hello, world!";

        assert!(verify_hmac_sha256(secret, body, "not-hex").is_err());
        assert!(verify_hmac_sha256(secret, body, "zz").is_err());
    }

    #[test]
    fn test_verify_hmac_sha256_empty_body() {
        let secret = b"my-webhook-secret";
        let body = b"";
        let sig = compute_hmac_sha256_hex(secret, body).unwrap();

        assert!(verify_hmac_sha256(secret, body, &sig).is_ok());
    }

    // RFC 4231 test vector 1 (known HMAC-SHA256 result)
    #[test]
    fn test_hmac_sha256_rfc4231_vector1() {
        let key = vec![0x0b_u8; 20]; // 20 bytes of 0x0b
        let data = b"Hi There";
        let expected_hex = "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7";

        let computed = compute_hmac_sha256_hex(&key, data).unwrap();
        assert_eq!(computed, expected_hex);
        assert!(verify_hmac_sha256(&key, data, expected_hex).is_ok());
    }

    // RFC 4231 test vector 2
    #[test]
    fn test_hmac_sha256_rfc4231_vector2() {
        let key = b"Jefe";
        let data = b"what do ya want for nothing?";
        let expected_hex = "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843";

        let computed = compute_hmac_sha256_hex(key, data).unwrap();
        assert_eq!(computed, expected_hex);
        assert!(verify_hmac_sha256(key, data, expected_hex).is_ok());
    }

    // -------------------------------------------------------------------
    // Bearer token verification
    // -------------------------------------------------------------------

    #[test]
    fn test_verify_bearer_token_valid() {
        let expected = "my-secret-token-12345";
        assert!(verify_bearer_token(expected, "my-secret-token-12345").is_ok());
    }

    #[test]
    fn test_verify_bearer_token_with_prefix() {
        let expected = "my-secret-token-12345";
        assert!(verify_bearer_token(expected, "Bearer my-secret-token-12345").is_ok());
    }

    #[test]
    fn test_verify_bearer_token_invalid() {
        let expected = "my-secret-token-12345";
        assert!(verify_bearer_token(expected, "wrong-token").is_err());
    }

    #[test]
    fn test_verify_bearer_token_wrong_prefix() {
        let expected = "my-secret-token-12345";
        // "Basic" prefix should fail
        assert!(verify_bearer_token(expected, "Basic my-secret-token-12345").is_err());
    }

    #[test]
    fn test_verify_bearer_token_empty() {
        let expected = "my-secret-token-12345";
        assert!(verify_bearer_token(expected, "").is_err());
    }

    // -------------------------------------------------------------------
    // Constant-time equality
    // -------------------------------------------------------------------

    #[test]
    fn test_constant_time_eq_equal() {
        assert!(constant_time_eq(b"hello", b"hello"));
    }

    #[test]
    fn test_constant_time_eq_not_equal() {
        assert!(!constant_time_eq(b"hello", b"world"));
    }

    #[test]
    fn test_constant_time_eq_different_lengths() {
        assert!(!constant_time_eq(b"short", b"longer string"));
    }

    #[test]
    fn test_constant_time_eq_empty() {
        assert!(constant_time_eq(b"", b""));
    }

    // -------------------------------------------------------------------
    // WebhookRegistry
    // -------------------------------------------------------------------

    #[test]
    fn test_registry_register_and_lookup() {
        let registry = WebhookRegistry::new();
        let wf_id = Uuid::now_v7();

        registry.register(
            "/trigger/test",
            WebhookConfig {
                workflow_id: wf_id,
                workflow_name: "test-wf".to_string(),
                auth: WebhookAuthMethod::None,
                when_clause: None,
            },
        );

        assert_eq!(registry.len(), 1);
        let config = registry.lookup("/trigger/test").unwrap();
        assert_eq!(config.workflow_id, wf_id);
    }

    #[test]
    fn test_registry_unregister() {
        let registry = WebhookRegistry::new();
        let wf_id = Uuid::now_v7();

        registry.register(
            "/trigger/test",
            WebhookConfig {
                workflow_id: wf_id,
                workflow_name: "test-wf".to_string(),
                auth: WebhookAuthMethod::None,
                when_clause: None,
            },
        );

        let removed = registry.unregister("/trigger/test");
        assert!(removed.is_some());
        assert_eq!(registry.len(), 0);
        assert!(registry.lookup("/trigger/test").is_err());
    }

    #[test]
    fn test_registry_lookup_not_found() {
        let registry = WebhookRegistry::new();
        assert!(registry.lookup("/nonexistent").is_err());
    }

    #[test]
    fn test_registry_path_normalization() {
        let registry = WebhookRegistry::new();
        let wf_id = Uuid::now_v7();

        // Register without leading slash
        registry.register(
            "trigger/test",
            WebhookConfig {
                workflow_id: wf_id,
                workflow_name: "test-wf".to_string(),
                auth: WebhookAuthMethod::None,
                when_clause: None,
            },
        );

        // Lookup with leading slash should find it
        let config = registry.lookup("/trigger/test").unwrap();
        assert_eq!(config.workflow_id, wf_id);
    }

    #[test]
    fn test_registry_verify_request_hmac() {
        let registry = WebhookRegistry::new();
        let wf_id = Uuid::now_v7();
        let secret = b"test-secret";

        registry.register(
            "/hook",
            WebhookConfig {
                workflow_id: wf_id,
                workflow_name: "test-wf".to_string(),
                auth: WebhookAuthMethod::HmacSha256 {
                    secret: secret.to_vec(),
                },
                when_clause: None,
            },
        );

        let body = b"request body";
        let sig = compute_hmac_sha256_hex(secret, body).unwrap();
        let prefixed = format!("sha256={sig}");

        // Valid signature should pass
        let config = registry
            .verify_request("/hook", body, Some(&prefixed), None)
            .unwrap();
        assert_eq!(config.workflow_id, wf_id);

        // Wrong signature should fail
        assert!(registry
            .verify_request("/hook", body, Some("sha256=wrong"), None)
            .is_err());

        // Missing signature header should fail
        assert!(registry
            .verify_request("/hook", body, None, None)
            .is_err());
    }

    #[test]
    fn test_registry_verify_request_bearer() {
        let registry = WebhookRegistry::new();
        let wf_id = Uuid::now_v7();

        registry.register(
            "/hook",
            WebhookConfig {
                workflow_id: wf_id,
                workflow_name: "test-wf".to_string(),
                auth: WebhookAuthMethod::BearerToken {
                    token: "secret-token".to_string(),
                },
                when_clause: None,
            },
        );

        // Valid bearer token
        let config = registry
            .verify_request("/hook", b"", None, Some("Bearer secret-token"))
            .unwrap();
        assert_eq!(config.workflow_id, wf_id);

        // Wrong token
        assert!(registry
            .verify_request("/hook", b"", None, Some("Bearer wrong-token"))
            .is_err());

        // Missing auth header
        assert!(registry
            .verify_request("/hook", b"", None, None)
            .is_err());
    }

    #[test]
    fn test_registry_verify_request_no_auth() {
        let registry = WebhookRegistry::new();
        let wf_id = Uuid::now_v7();

        registry.register(
            "/hook",
            WebhookConfig {
                workflow_id: wf_id,
                workflow_name: "test-wf".to_string(),
                auth: WebhookAuthMethod::None,
                when_clause: None,
            },
        );

        // No auth required, should pass
        let config = registry
            .verify_request("/hook", b"anything", None, None)
            .unwrap();
        assert_eq!(config.workflow_id, wf_id);
    }

    #[test]
    fn test_registry_paths_listing() {
        let registry = WebhookRegistry::new();

        registry.register(
            "/hook/one",
            WebhookConfig {
                workflow_id: Uuid::now_v7(),
                workflow_name: "wf-1".to_string(),
                auth: WebhookAuthMethod::None,
                when_clause: None,
            },
        );
        registry.register(
            "/hook/two",
            WebhookConfig {
                workflow_id: Uuid::now_v7(),
                workflow_name: "wf-2".to_string(),
                auth: WebhookAuthMethod::None,
                when_clause: None,
            },
        );

        let paths = registry.paths();
        assert_eq!(paths.len(), 2);
        assert!(paths.contains(&"/hook/one".to_string()));
        assert!(paths.contains(&"/hook/two".to_string()));
    }

    // -------------------------------------------------------------------
    // hex helpers
    // -------------------------------------------------------------------

    #[test]
    fn test_hex_encode_decode_roundtrip() {
        let data = b"Hello, World!";
        let hex = hex_encode(data);
        let decoded = hex_decode(&hex).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_hex_decode_invalid() {
        assert!(hex_decode("0").is_err()); // Odd length
        assert!(hex_decode("zz").is_err()); // Invalid chars
    }

    // -------------------------------------------------------------------
    // normalize_path
    // -------------------------------------------------------------------

    #[test]
    fn test_normalize_path_adds_leading_slash() {
        assert_eq!(normalize_path("trigger/test"), "/trigger/test");
    }

    #[test]
    fn test_normalize_path_removes_trailing_slash() {
        assert_eq!(normalize_path("/trigger/test/"), "/trigger/test");
    }

    #[test]
    fn test_normalize_path_root() {
        assert_eq!(normalize_path("/"), "/");
    }

    #[test]
    fn test_normalize_path_already_correct() {
        assert_eq!(normalize_path("/trigger/test"), "/trigger/test");
    }
}
