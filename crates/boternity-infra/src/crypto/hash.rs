//! SHA-256 content hashing for SOUL.md integrity verification.
//!
//! Implements the `ContentHasher` trait from `boternity-core` using the
//! `sha2` crate (RustCrypto ecosystem).

use sha2::{Digest, Sha256};

use boternity_core::service::hash::ContentHasher;

/// SHA-256 implementation of `ContentHasher`.
///
/// Computes lowercase hex-encoded SHA-256 digests of content strings.
/// Used to verify SOUL.md integrity at bot startup.
pub struct Sha256ContentHasher;

impl Sha256ContentHasher {
    /// Create a new hasher.
    pub fn new() -> Self {
        Self
    }
}

impl Default for Sha256ContentHasher {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentHasher for Sha256ContentHasher {
    fn compute_hash(&self, content: &str) -> String {
        let digest = Sha256::digest(content.as_bytes());
        format!("{:x}", digest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256_hash_known_value() {
        let hasher = Sha256ContentHasher::new();
        // SHA-256 of empty string
        let hash = hasher.compute_hash("");
        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_sha256_hash_deterministic() {
        let hasher = Sha256ContentHasher::new();
        let content = "# Luna\nCurious and warm.";
        let hash1 = hasher.compute_hash(content);
        let hash2 = hasher.compute_hash(content);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_sha256_hash_different_content() {
        let hasher = Sha256ContentHasher::new();
        let hash1 = hasher.compute_hash("content A");
        let hash2 = hasher.compute_hash("content B");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_sha256_hash_is_lowercase_hex() {
        let hasher = Sha256ContentHasher::new();
        let hash = hasher.compute_hash("test");
        assert_eq!(hash.len(), 64); // SHA-256 = 32 bytes = 64 hex chars
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(hash.chars().all(|c| !c.is_ascii_uppercase()));
    }
}
