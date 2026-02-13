//! Wasmtime WASM runtime configuration and component loading.
//!
//! Provides [`WasmRuntime`] which manages separate Wasmtime engines for
//! different trust tiers. Each engine is configured with fuel consumption,
//! epoch interruption, and the Component Model.

use anyhow::Result;
use boternity_types::skill::{ResourceLimits, TrustTier};
use wasmtime::{component::Component, Config, Engine};

// Generate Rust bindings from the WIT skill-plugin world.
// Async behavior is governed by the engine's async_support(true) config,
// not by the bindgen! macro in wasmtime v40.
wasmtime::component::bindgen!({
    world: "skill-plugin",
    path: "../../wit/boternity-skill.wit",
});

/// Wasmtime-based WASM runtime with per-trust-tier engine configurations.
///
/// Separate engines are used for verified and untrusted trust tiers to enforce
/// different security policies (anti-pattern to share a single engine for all
/// trust levels).
pub struct WasmRuntime {
    verified_engine: Engine,
    untrusted_engine: Engine,
}

impl WasmRuntime {
    /// Create a new runtime with engines for verified and untrusted tiers.
    ///
    /// # Errors
    ///
    /// Returns an error if engine creation fails (e.g., unsupported platform).
    pub fn new() -> Result<Self> {
        let verified_config = Self::create_engine_config(&TrustTier::Verified);
        let untrusted_config = Self::create_engine_config(&TrustTier::Untrusted);

        Ok(Self {
            verified_engine: Engine::new(&verified_config)?,
            untrusted_engine: Engine::new(&untrusted_config)?,
        })
    }

    /// Build a Wasmtime [`Config`] tuned for the given trust tier.
    ///
    /// All tiers get: async support, Component Model, fuel consumption, epoch
    /// interruption. Untrusted additionally disables SIMD.
    fn create_engine_config(trust_tier: &TrustTier) -> Config {
        let mut config = Config::new();

        // Common settings for all sandboxed tiers
        config.async_support(true);
        config.wasm_component_model(true);
        config.consume_fuel(true);
        config.epoch_interruption(true);

        // Threads always disabled for both tiers (skills are single-threaded)
        config.wasm_threads(false);

        match trust_tier {
            TrustTier::Verified => {
                config.wasm_simd(true);
            }
            TrustTier::Untrusted => {
                // Must disable relaxed-SIMD before SIMD (relaxed depends on SIMD).
                config.wasm_relaxed_simd(false);
                config.wasm_simd(false);
            }
            TrustTier::Local => {
                // Local skills run natively, not in WASM sandbox.
                // This branch should never be reached in normal operation.
                panic!("Local trust tier does not use WASM engine");
            }
        }

        config
    }

    /// Get the engine configured for the given trust tier.
    ///
    /// # Panics
    ///
    /// Panics if `tier` is [`TrustTier::Local`] since local skills do not run
    /// in a WASM sandbox.
    pub fn engine_for_tier(&self, tier: &TrustTier) -> &Engine {
        match tier {
            TrustTier::Verified => &self.verified_engine,
            TrustTier::Untrusted => &self.untrusted_engine,
            TrustTier::Local => {
                panic!("Local trust tier does not use WASM engine");
            }
        }
    }

    /// Load and validate a WASM component from raw bytes.
    ///
    /// The component is compiled using the engine for the specified trust tier.
    ///
    /// # Errors
    ///
    /// Returns an error if the bytes are not a valid WASM component or
    /// compilation fails.
    pub fn load_component(&self, tier: &TrustTier, wasm_bytes: &[u8]) -> Result<Component> {
        let engine = self.engine_for_tier(tier);
        Component::new(engine, wasm_bytes)
    }

    /// Get default resource limits for a trust tier.
    ///
    /// Untrusted skills get stricter limits than verified ones.
    pub fn default_resource_limits(tier: &TrustTier) -> ResourceLimits {
        match tier {
            TrustTier::Untrusted => ResourceLimits {
                max_memory_bytes: 16 * 1024 * 1024, // 16 MB
                max_fuel: 500_000,
                max_duration_ms: 10_000, // 10s
            },
            TrustTier::Verified => ResourceLimits {
                max_memory_bytes: 64 * 1024 * 1024, // 64 MB
                max_fuel: 1_000_000,
                max_duration_ms: 30_000, // 30s
            },
            TrustTier::Local => ResourceLimits {
                // Local skills are not sandboxed; limits are advisory.
                max_memory_bytes: 256 * 1024 * 1024, // 256 MB
                max_fuel: u64::MAX,
                max_duration_ms: 300_000, // 5 min
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wasm_runtime_new_succeeds() {
        let runtime = WasmRuntime::new();
        assert!(runtime.is_ok(), "WasmRuntime::new() should succeed");
    }

    #[test]
    fn engine_for_tier_returns_different_engines() {
        let runtime = WasmRuntime::new().expect("runtime creation should succeed");

        let verified = runtime.engine_for_tier(&TrustTier::Verified);
        let untrusted = runtime.engine_for_tier(&TrustTier::Untrusted);

        // Different engine instances have different pointers
        assert!(
            !std::ptr::eq(verified, untrusted),
            "verified and untrusted engines must be distinct instances"
        );
    }

    #[test]
    fn default_resource_limits_stricter_for_untrusted() {
        let untrusted = WasmRuntime::default_resource_limits(&TrustTier::Untrusted);
        let verified = WasmRuntime::default_resource_limits(&TrustTier::Verified);

        assert!(
            untrusted.max_memory_bytes < verified.max_memory_bytes,
            "untrusted memory limit ({}) should be less than verified ({})",
            untrusted.max_memory_bytes,
            verified.max_memory_bytes
        );
        assert!(
            untrusted.max_fuel < verified.max_fuel,
            "untrusted fuel limit ({}) should be less than verified ({})",
            untrusted.max_fuel,
            verified.max_fuel
        );
        assert!(
            untrusted.max_duration_ms < verified.max_duration_ms,
            "untrusted duration limit ({}) should be less than verified ({})",
            untrusted.max_duration_ms,
            verified.max_duration_ms
        );
    }

    #[test]
    #[should_panic(expected = "Local trust tier does not use WASM engine")]
    fn engine_for_tier_local_panics() {
        let runtime = WasmRuntime::new().expect("runtime creation should succeed");
        let _ = runtime.engine_for_tier(&TrustTier::Local);
    }
}
