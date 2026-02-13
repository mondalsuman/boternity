//! Shared workspace for passing data between agents in a hierarchy.
//!
//! `SharedWorkspace` is a concurrent key-value store backed by `DashMap`.
//! Values are cloned on read to avoid holding a `DashMap` `Ref` across
//! `.await` points, which would deadlock.

use std::sync::Arc;

use dashmap::DashMap;
use serde_json::Value;

/// Concurrent key-value workspace shared across an agent hierarchy.
///
/// Cloning produces a shared view of the same underlying data (backed by `Arc`).
/// All reads return cloned values -- never hold a `DashMap` guard across await.
#[derive(Debug, Clone)]
pub struct SharedWorkspace {
    inner: Arc<DashMap<String, Value>>,
}

impl SharedWorkspace {
    /// Create an empty workspace.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(DashMap::new()),
        }
    }

    /// Get a cloned copy of the value at `key`, or `None` if absent.
    ///
    /// The value is cloned immediately so no `DashMap` guard is held after return.
    pub fn get(&self, key: &str) -> Option<Value> {
        self.inner.get(key).map(|r| r.value().clone())
    }

    /// Insert or overwrite a key-value pair.
    pub fn set(&self, key: String, value: Value) {
        self.inner.insert(key, value);
    }

    /// Remove a key and return its value, if present.
    pub fn remove(&self, key: &str) -> Option<Value> {
        self.inner.remove(key).map(|(_, v)| v)
    }

    /// Check whether a key exists.
    pub fn contains(&self, key: &str) -> bool {
        self.inner.contains_key(key)
    }

    /// Snapshot of all current keys.
    pub fn keys(&self) -> Vec<String> {
        self.inner.iter().map(|r| r.key().clone()).collect()
    }

    /// Number of entries in the workspace.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Whether the workspace is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl Default for SharedWorkspace {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn set_get_roundtrip() {
        let ws = SharedWorkspace::new();
        ws.set("key1".to_string(), json!("hello"));
        assert_eq!(ws.get("key1"), Some(json!("hello")));
    }

    #[test]
    fn get_missing_returns_none() {
        let ws = SharedWorkspace::new();
        assert_eq!(ws.get("missing"), None);
    }

    #[test]
    fn set_overwrites() {
        let ws = SharedWorkspace::new();
        ws.set("k".to_string(), json!(1));
        ws.set("k".to_string(), json!(2));
        assert_eq!(ws.get("k"), Some(json!(2)));
    }

    #[test]
    fn remove_returns_value() {
        let ws = SharedWorkspace::new();
        ws.set("k".to_string(), json!("v"));
        assert_eq!(ws.remove("k"), Some(json!("v")));
        assert_eq!(ws.get("k"), None);
    }

    #[test]
    fn remove_missing_returns_none() {
        let ws = SharedWorkspace::new();
        assert_eq!(ws.remove("k"), None);
    }

    #[test]
    fn contains_and_keys() {
        let ws = SharedWorkspace::new();
        ws.set("a".to_string(), json!(1));
        ws.set("b".to_string(), json!(2));
        assert!(ws.contains("a"));
        assert!(!ws.contains("c"));
        let mut keys = ws.keys();
        keys.sort();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn len_and_is_empty() {
        let ws = SharedWorkspace::new();
        assert!(ws.is_empty());
        assert_eq!(ws.len(), 0);
        ws.set("k".to_string(), json!(1));
        assert!(!ws.is_empty());
        assert_eq!(ws.len(), 1);
    }

    #[tokio::test]
    async fn concurrent_access_no_panic() {
        let ws = SharedWorkspace::new();
        let mut handles = Vec::new();

        for i in 0..50 {
            let ws_clone = ws.clone();
            handles.push(tokio::spawn(async move {
                ws_clone.set(format!("key-{i}"), json!(i));
                let _ = ws_clone.get(&format!("key-{i}"));
            }));
        }

        for handle in handles {
            handle.await.unwrap();
        }

        assert_eq!(ws.len(), 50);
    }

    #[test]
    fn clone_shares_data() {
        let ws = SharedWorkspace::new();
        let ws2 = ws.clone();
        ws.set("shared".to_string(), json!("data"));
        assert_eq!(ws2.get("shared"), Some(json!("data")));
    }
}
