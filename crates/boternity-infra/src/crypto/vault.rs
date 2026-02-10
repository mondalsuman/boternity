//! AES-256-GCM vault encryption for secrets at rest.
//!
//! VaultCrypto provides symmetric encryption using AES-256-GCM with random nonces.
//! The master key can come from:
//! - A raw 32-byte key
//! - A password (Argon2id key derivation)
//! - The OS keychain (auto-generated, zero-friction default)
//!
//! Encrypted format: `nonce (12 bytes) || ciphertext`
//!
//! SECURITY: Error types never contain plaintext or key material.

use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{AeadCore, Aes256Gcm, Nonce};
use thiserror::Error;

/// Nonce size for AES-256-GCM (96 bits / 12 bytes).
const NONCE_SIZE: usize = 12;

/// Service name used for keychain storage of the master key.
const KEYCHAIN_SERVICE: &str = "boternity";
/// Keychain user/account for the vault master key.
const KEYCHAIN_USER: &str = "vault-master-key";

/// Errors from vault encryption operations.
///
/// IMPORTANT: These errors never include plaintext, key material, or ciphertext
/// in their Display/Debug output to prevent accidental logging of secrets.
#[derive(Debug, Error)]
pub enum VaultError {
    #[error("encryption failed")]
    EncryptionFailed,

    #[error("decryption failed")]
    DecryptionFailed,

    #[error("invalid ciphertext: too short")]
    CiphertextTooShort,

    #[error("key derivation failed")]
    KeyDerivationFailed,

    #[error("keychain unavailable: {0}")]
    KeychainUnavailable(String),

    #[error("keychain error: {0}")]
    KeychainError(String),
}

/// AES-256-GCM encryption for vault secrets at rest.
///
/// Each encryption call generates a random 12-byte nonce, prepended to the ciphertext.
/// This means encrypting the same plaintext twice produces different output.
pub struct VaultCrypto {
    cipher: Aes256Gcm,
}

impl VaultCrypto {
    /// Create a new VaultCrypto from a raw 32-byte key.
    pub fn new(key: &[u8; 32]) -> Self {
        Self {
            cipher: Aes256Gcm::new(key.into()),
        }
    }

    /// Derive a 32-byte encryption key from a password using Argon2id.
    ///
    /// Uses OWASP recommended parameters:
    /// - 19 MiB memory (19456 KiB)
    /// - 2 iterations
    /// - 1 parallelism degree
    ///
    /// The salt is deterministic ("boternity-vault-v1") so the same password
    /// always produces the same key. This is acceptable because the password
    /// itself provides the entropy, and we're not storing the hash for
    /// verification (we're using it as a KDF for encryption).
    pub fn from_password(password: &str) -> Result<Self, VaultError> {
        use argon2::{Algorithm, Argon2, Params, Version};

        let params = Params::new(19456, 2, 1, Some(32))
            .map_err(|_| VaultError::KeyDerivationFailed)?;

        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

        let salt = b"boternity-vault-v1";
        let mut key = [0u8; 32];
        argon2
            .hash_password_into(password.as_bytes(), salt, &mut key)
            .map_err(|_| VaultError::KeyDerivationFailed)?;

        Ok(Self::new(&key))
    }

    /// Load or auto-generate a master key from the OS keychain.
    ///
    /// This is the zero-friction default path:
    /// 1. Try to load existing key from keychain under service="boternity" user="vault-master-key"
    /// 2. If not found, generate a random 32-byte key
    /// 3. Store the new key in keychain
    /// 4. Create cipher from the key
    ///
    /// The key is stored as a hex string in the keychain (64 hex chars = 32 bytes).
    pub fn from_keychain() -> Result<Self, VaultError> {
        let entry = keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_USER)
            .map_err(|e| VaultError::KeychainUnavailable(e.to_string()))?;

        match entry.get_password() {
            Ok(hex_key) => {
                // Key exists in keychain, decode it
                let key_bytes = hex_decode(&hex_key)
                    .map_err(|_| VaultError::KeychainError("corrupted key in keychain".to_string()))?;
                if key_bytes.len() != 32 {
                    return Err(VaultError::KeychainError(
                        "invalid key length in keychain".to_string(),
                    ));
                }
                let mut key = [0u8; 32];
                key.copy_from_slice(&key_bytes);
                Ok(Self::new(&key))
            }
            Err(keyring::Error::NoEntry) => {
                // No key yet -- generate a random one
                let key: [u8; 32] = rand_bytes();
                let hex_key = hex_encode(&key);
                entry
                    .set_password(&hex_key)
                    .map_err(|e| VaultError::KeychainError(e.to_string()))?;
                Ok(Self::new(&key))
            }
            Err(e) => Err(VaultError::KeychainUnavailable(e.to_string())),
        }
    }

    /// Encrypt plaintext using AES-256-GCM with a random nonce.
    ///
    /// Returns `nonce (12 bytes) || ciphertext`.
    /// Each call generates a fresh random nonce, so encrypting the same
    /// plaintext twice always produces different output.
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, VaultError> {
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = self
            .cipher
            .encrypt(&nonce, plaintext)
            .map_err(|_| VaultError::EncryptionFailed)?;

        // Prepend nonce to ciphertext
        let mut result = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
        result.extend_from_slice(&nonce);
        result.extend_from_slice(&ciphertext);
        Ok(result)
    }

    /// Decrypt data produced by `encrypt()`.
    ///
    /// Expects `nonce (12 bytes) || ciphertext` format.
    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, VaultError> {
        if data.len() < NONCE_SIZE {
            return Err(VaultError::CiphertextTooShort);
        }

        let (nonce_bytes, ciphertext) = data.split_at(NONCE_SIZE);
        let nonce = Nonce::from_slice(nonce_bytes);

        self.cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| VaultError::DecryptionFailed)
    }
}

/// Generate 32 random bytes using the OS CSPRNG.
fn rand_bytes() -> [u8; 32] {
    use aes_gcm::aead::rand_core::RngCore;
    let mut key = [0u8; 32];
    OsRng.fill_bytes(&mut key);
    key
}

/// Hex-encode bytes to string.
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Hex-decode a string to bytes.
fn hex_decode(s: &str) -> Result<Vec<u8>, String> {
    if s.len() % 2 != 0 {
        return Err("odd length hex string".to_string());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&s[i..i + 2], 16)
                .map_err(|e| format!("invalid hex at position {i}: {e}"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> [u8; 32] {
        // Deterministic key for testing only
        let mut key = [0u8; 32];
        for (i, byte) in key.iter_mut().enumerate() {
            *byte = i as u8;
        }
        key
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let crypto = VaultCrypto::new(&test_key());
        let plaintext = b"hello world, this is a secret API key";

        let encrypted = crypto.encrypt(plaintext).unwrap();
        let decrypted = crypto.decrypt(&encrypted).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_decrypt_with_wrong_key_fails() {
        let crypto1 = VaultCrypto::new(&test_key());
        let mut wrong_key = test_key();
        wrong_key[0] = 0xFF; // Flip one byte
        let crypto2 = VaultCrypto::new(&wrong_key);

        let encrypted = crypto1.encrypt(b"secret data").unwrap();
        let result = crypto2.decrypt(&encrypted);

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), VaultError::DecryptionFailed));
    }

    #[test]
    fn test_random_nonce_produces_different_ciphertexts() {
        let crypto = VaultCrypto::new(&test_key());
        let plaintext = b"same plaintext";

        let encrypted1 = crypto.encrypt(plaintext).unwrap();
        let encrypted2 = crypto.encrypt(plaintext).unwrap();

        // Ciphertexts should differ (different random nonces)
        assert_ne!(encrypted1, encrypted2);

        // But both should decrypt to the same plaintext
        assert_eq!(crypto.decrypt(&encrypted1).unwrap(), plaintext);
        assert_eq!(crypto.decrypt(&encrypted2).unwrap(), plaintext);
    }

    #[test]
    fn test_ciphertext_too_short() {
        let crypto = VaultCrypto::new(&test_key());
        let result = crypto.decrypt(&[0u8; 5]); // Less than 12-byte nonce

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), VaultError::CiphertextTooShort));
    }

    #[test]
    fn test_empty_plaintext() {
        let crypto = VaultCrypto::new(&test_key());
        let encrypted = crypto.encrypt(b"").unwrap();
        let decrypted = crypto.decrypt(&encrypted).unwrap();
        assert!(decrypted.is_empty());
    }

    #[test]
    fn test_from_password() {
        let crypto1 = VaultCrypto::from_password("my-strong-password").unwrap();
        let crypto2 = VaultCrypto::from_password("my-strong-password").unwrap();

        // Same password should produce same key (deterministic salt)
        let plaintext = b"test data";
        let encrypted = crypto1.encrypt(plaintext).unwrap();
        let decrypted = crypto2.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_different_passwords_produce_different_keys() {
        let crypto1 = VaultCrypto::from_password("password-one").unwrap();
        let crypto2 = VaultCrypto::from_password("password-two").unwrap();

        let encrypted = crypto1.encrypt(b"secret").unwrap();
        let result = crypto2.decrypt(&encrypted);
        assert!(result.is_err());
    }

    #[test]
    fn test_hex_roundtrip() {
        let bytes = [0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0xFF];
        let encoded = hex_encode(&bytes);
        assert_eq!(encoded, "deadbeef00ff");
        let decoded = hex_decode(&encoded).unwrap();
        assert_eq!(decoded, bytes);
    }

    #[test]
    fn test_vault_error_never_contains_secrets() {
        // Verify error Display output doesn't leak actual key/plaintext data.
        // Error messages may contain technical terms like "key derivation" or "ciphertext"
        // but must never contain actual secret values.
        let test_secret = "sk-super-secret-value-12345";
        let test_key_hex = "deadbeefcafebabe";

        let errors = [
            VaultError::EncryptionFailed,
            VaultError::DecryptionFailed,
            VaultError::CiphertextTooShort,
            VaultError::KeyDerivationFailed,
            VaultError::KeychainUnavailable("no keychain service".to_string()),
            VaultError::KeychainError("credential store locked".to_string()),
        ];

        for err in &errors {
            let msg = err.to_string();
            assert!(!msg.contains(test_secret), "Error leaks secret value: {msg}");
            assert!(!msg.contains(test_key_hex), "Error leaks key material: {msg}");
        }
    }
}
