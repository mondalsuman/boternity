//! Provider registry for runtime provider lookup.
//!
//! A simple name-indexed registry of boxed LLM providers.

use std::collections::HashMap;

use super::box_provider::BoxLlmProvider;

/// Registry of available LLM providers, indexed by name.
///
/// Used for runtime provider lookup when building a `FallbackChain`
/// or selecting a specific provider by name.
pub struct ProviderRegistry {
    providers: HashMap<String, BoxLlmProvider>,
}

impl ProviderRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    /// Register a provider under the given name.
    ///
    /// If a provider with this name already exists, it is replaced.
    pub fn register(&mut self, name: impl Into<String>, provider: BoxLlmProvider) {
        self.providers.insert(name.into(), provider);
    }

    /// Look up a provider by name.
    pub fn get(&self, name: &str) -> Option<&BoxLlmProvider> {
        self.providers.get(name)
    }

    /// List all registered provider names.
    pub fn list_names(&self) -> Vec<&str> {
        self.providers.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}
