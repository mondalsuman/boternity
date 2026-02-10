---
phase: 01-foundation-bot-identity
plan: 04
subsystem: infra
tags: [rust, aes-gcm, keyring, encryption, secrets, vault, keychain, env-vars, resolution-chain]

# Dependency graph
requires:
  - "01-01 (domain types: SecretKey, SecretEntry, SecretScope, Redacted, SecretProvider trait)"
  - "01-02 (SqliteSecretRepository for vault persistence, DatabasePool)"
provides:
  - "AES-256-GCM vault encryption via VaultCrypto (password-based and keychain-based key derivation)"
  - "OS keychain secret storage via KeychainProvider (keyring crate)"
  - "Environment variable secret provider (EnvSecretProvider)"
  - "VaultSecretProvider combining SQLite + AES-256-GCM encryption"
  - "SecretService with resolution chain (env > per-bot > global vault)"
  - "SecretChain builder wiring concrete providers in infra"
  - "Secret<T> generic wrapper with redacted Debug/Display"
  - "BoxSecretProvider object-safe trait for dynamic dispatch"
affects:
  - "01-05 (CLI and REST API will use SecretService for bnity set/list/check secrets)"
  - "Phase 2+ (Bots need API keys resolved through SecretService)"

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "BoxSecretProvider blanket impl for object-safe dynamic dispatch of RPITIT traits"
    - "VaultCrypto nonce||ciphertext format for AES-256-GCM encrypted at rest"
    - "Hex encoding for encrypted BLOB transport through string-based interfaces"
    - "Secret chain precedence: env > keychain > vault"
    - "Zero-friction default: auto-generated master key in OS keychain"

key-files:
  created:
    - "crates/boternity-infra/src/crypto/vault.rs"
    - "crates/boternity-infra/src/crypto/mod.rs"
    - "crates/boternity-infra/src/keychain/mod.rs"
    - "crates/boternity-infra/src/secret/mod.rs"
    - "crates/boternity-infra/src/secret/env.rs"
    - "crates/boternity-infra/src/secret/chain.rs"
    - "crates/boternity-core/src/service/secret.rs"
  modified:
    - "crates/boternity-infra/src/lib.rs"
    - "crates/boternity-core/src/service/mod.rs"
    - "crates/boternity-core/src/repository/secret.rs"
    - "crates/boternity-core/Cargo.toml"
    - "crates/boternity-types/src/secret.rs"
    - "Cargo.lock"

key-decisions:
  - "BoxSecretProvider with blanket impl for object-safe dynamic dispatch (RPITIT traits are not object-safe)"
  - "Fixed salt 'boternity-vault-v1' for Argon2id password KDF (acceptable since password provides entropy)"
  - "Keychain master key stored as hex string (64 hex chars = 32 bytes) under service='boternity' user='vault-master-key'"
  - "Secret<T> generic wrapper alongside existing Redacted(String) for type-safe redaction"
  - "tokio added as dev-dependency to boternity-core for async test support"

patterns-established:
  - "Object-safe companion trait pattern: SecretProvider (RPITIT) + BoxSecretProvider (Pin<Box<dyn Future>>) with blanket impl"
  - "Provider chain pattern: iterate providers in order, first match wins, skip read-only on writes"
  - "Bot scope fallback: try bot-scoped first across all providers, then fall back to global"
  - "Secret masking: show last 4 chars or **** for short values"

# Metrics
duration: 11min 25s
completed: 2026-02-10
---

# Phase 1 Plan 4: Secrets Vault + Resolution Chain Summary

**AES-256-GCM vault encryption with OS keychain auto-generated master key, env var override, and SecretService resolution chain (env > per-bot > global vault) using BoxSecretProvider for dynamic dispatch**

## Performance

- **Duration:** 11 min 25s
- **Started:** 2026-02-10T21:34:08Z
- **Completed:** 2026-02-10T21:45:33Z
- **Tasks:** 2/2
- **Files created:** 7
- **Files modified:** 6
- **Tests added:** 39 new tests (17 infra vault/provider + 16 core service + 5 types Secret<T> + 1 doctest)

## Accomplishments

- VaultCrypto with AES-256-GCM encryption using random nonces, password-based Argon2id key derivation, and auto-generated keychain master key
- Three secret provider backends: EnvSecretProvider (read-only), KeychainProvider (OS keychain via keyring), VaultSecretProvider (SQLite + encryption)
- SecretService with resolution chain implementing env > per-bot > global vault precedence
- BoxSecretProvider object-safe trait with blanket implementation enabling dynamic dispatch for RPITIT traits
- Secret<T> generic wrapper that always prints "***REDACTED***" in Debug/Display output
- SecretChain builder in infra layer wiring concrete providers without polluting core
- boternity-core maintains zero infra dependencies (verified via cargo tree)

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement vault encryption and secret provider backends** - `1092391` (feat)
2. **Task 2: Implement SecretService with resolution chain** - `ddfe9e5` (feat)

## Files Created/Modified

- `crates/boternity-infra/src/crypto/vault.rs` - AES-256-GCM VaultCrypto: encrypt/decrypt, from_password, from_keychain
- `crates/boternity-infra/src/crypto/mod.rs` - Crypto module declarations
- `crates/boternity-infra/src/keychain/mod.rs` - KeychainProvider wrapping keyring crate with scoped key prefixes
- `crates/boternity-infra/src/secret/mod.rs` - VaultSecretProvider combining SqliteSecretRepository + VaultCrypto
- `crates/boternity-infra/src/secret/env.rs` - EnvSecretProvider for environment variable secrets (read-only)
- `crates/boternity-infra/src/secret/chain.rs` - build_secret_chain() wiring concrete providers in priority order
- `crates/boternity-infra/src/lib.rs` - Added crypto, keychain, secret module declarations
- `crates/boternity-core/src/service/secret.rs` - SecretService with provider chain, masking, and mock-based tests
- `crates/boternity-core/src/service/mod.rs` - Added pub mod secret declaration
- `crates/boternity-core/src/repository/secret.rs` - Added BoxSecretProvider object-safe trait with blanket impl
- `crates/boternity-core/Cargo.toml` - Added tokio dev-dependency for async tests
- `crates/boternity-types/src/secret.rs` - Added Secret<T> generic wrapper with redacted Debug/Display
- `Cargo.lock` - Updated with new dev-dependency

## Decisions Made

- **BoxSecretProvider pattern:** The existing SecretProvider trait uses RPITIT (impl Future), which is NOT object-safe in Rust 2024. Created BoxSecretProvider with Pin<Box<dyn Future>> returns and a blanket impl so any SecretProvider automatically gets object-safe dispatch. This allows SecretService to use Vec<Arc<dyn BoxSecretProvider>> for the provider chain.
- **Fixed Argon2id salt:** Used "boternity-vault-v1" as deterministic salt for password-based key derivation. Acceptable because password provides the entropy and we're using it as a KDF (not storing hashes for verification).
- **Secret<T> alongside Redacted:** Added generic Secret<T> without removing existing Redacted(String). Both serve the same purpose; Secret<T> is more flexible for non-string secrets.
- **Parallel execution coordination:** Committed only module declarations owned by this plan. Plan 03 (running in parallel) owns filesystem/, crypto/hash.rs, and service/bot.rs. Shared files (lib.rs, service/mod.rs, crypto/mod.rs) modified minimally.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed unsafe env var calls in tests (Rust 2024 edition)**
- **Found during:** Task 1 (EnvSecretProvider tests)
- **Issue:** `std::env::set_var` and `std::env::remove_var` are unsafe in Rust 2024 edition. Compilation failed.
- **Fix:** Wrapped calls in `unsafe {}` blocks with safety documentation comments.
- **Files modified:** crates/boternity-infra/src/secret/env.rs
- **Verification:** Tests compile and pass
- **Committed in:** 1092391 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Trivial Rust 2024 edition change. No scope creep.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- SecretService ready for Plan 01-05 (CLI `bnity set secret`, `bnity list secrets`, `bnity check`)
- VaultCrypto + keychain integration ready for zero-friction default experience
- Resolution chain wired and tested: env vars override everything, per-bot overrides global
- Secret<T> wrapper available for any code handling sensitive values
- All provider backends tested independently and through VaultSecretProvider integration

## Self-Check: PASSED

---
*Phase: 01-foundation-bot-identity*
*Completed: 2026-02-10*
