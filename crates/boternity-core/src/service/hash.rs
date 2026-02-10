//! ContentHasher trait for computing integrity hashes.
//!
//! Defined in boternity-core so services can hash content without coupling to
//! a specific hashing algorithm. The `Sha256ContentHasher` adapter lives in
//! boternity-infra.

/// Abstraction over content hashing for integrity verification.
///
/// Used by SoulService to compute SHA-256 hashes of SOUL.md content and
/// verify integrity at bot startup.
pub trait ContentHasher: Send + Sync {
    /// Compute a hex-encoded hash of the given content.
    fn compute_hash(&self, content: &str) -> String;
}
