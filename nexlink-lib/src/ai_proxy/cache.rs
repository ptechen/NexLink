//! Cache module for AI proxy application
//! Implements intelligent caching for AI responses to optimize performance and reduce costs

use std::collections::HashMap;
use std::hash::Hash;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};
use tracing::{info, debug};

/// Cache entry with expiration time
#[derive(Debug, Clone)]
struct CacheEntry<T> {
    value: T,
    expires_at: Instant,
    created_at: Instant,
}

impl<T> CacheEntry<T> {
    fn new(value: T, ttl: Duration) -> Self {
        let created_at = Instant::now();
        let expires_at = created_at + ttl;
        Self {
            value,
            expires_at,
            created_at,
        }
    }

    fn is_expired(&self) -> bool {
        Instant::now() >= self.expires_at
    }
}

/// Configuration for cache behavior
#[derive(Debug, Clone)]
pub struct CacheConfig {
    pub default_ttl: Duration,           // Default time-to-live for cache entries
    pub max_size: usize,                // Maximum number of entries in cache
    pub eviction_batch_size: usize,     // Number of entries to evict when max_size reached
    pub cleanup_interval: Duration,     // How often to clean up expired entries
    pub enable_compression: bool,       // Whether to enable compression for cached values
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            default_ttl: Duration::from_secs(300),  // 5 minutes
            max_size: 1000,                        // 1000 entries
            eviction_batch_size: 100,              // Remove 100 oldest when over capacity
            cleanup_interval: Duration::from_secs(60), // Clean up every minute
            enable_compression: false,
        }
    }
}

/// Intelligent cache for AI responses
pub struct AiCache<K, V> {
    entries: RwLock<HashMap<K, CacheEntry<V>>>,
    config: CacheConfig,
    hits: RwLock<u64>,
    misses: RwLock<u64>,
}

impl<K, V> AiCache<K, V>
where
    K: Eq + Hash + Clone + std::fmt::Debug,
    V: Clone,
{
    pub fn new(config: CacheConfig) -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            config,
            hits: RwLock::new(0),
            misses: RwLock::new(0),
        }
    }

    /// Insert a value into the cache with a specific TTL
    pub async fn put(&self, key: K, value: V, ttl: Option<Duration>) -> Result<(), String> {
        let ttl = ttl.unwrap_or(self.config.default_ttl);
        let entry = CacheEntry::new(value, ttl);

        let mut entries = self.entries.write().await;

        // Check if we need to evict entries
        if entries.len() >= self.config.max_size {
            self.evict_entries(&mut entries).await;
        }

        entries.insert(key, entry);
        debug!("Cached entry inserted");

        Ok(())
    }

    /// Retrieve a value from the cache
    pub async fn get(&self, key: &K) -> Option<V> {
        let entries = self.entries.read().await;
        if let Some(entry) = entries.get(key) {
            if !entry.is_expired() {
                // Update hit counter
                *self.hits.write().await += 1;
                debug!("Cache hit for key: {:?}", key);
                Some(entry.value.clone())
            } else {
                // Entry is expired but we haven't cleaned it up yet
                debug!("Expired cache entry for key: {:?}", key);
                None
            }
        } else {
            // Update miss counter
            *self.misses.write().await += 1;
            debug!("Cache miss for key: {:?}", key);
            None
        }
    }

    /// Check if a key exists and is not expired
    pub async fn contains_key(&self, key: &K) -> bool {
        let entries = self.entries.read().await;
        if let Some(entry) = entries.get(key) {
            !entry.is_expired()
        } else {
            false
        }
    }

    /// Remove a specific key from the cache
    pub async fn remove(&self, key: &K) -> Option<V> {
        let mut entries = self.entries.write().await;
        if let Some(entry) = entries.remove(key) {
            Some(entry.value)
        } else {
            None
        }
    }

    /// Clear all entries from the cache
    pub async fn clear(&self) {
        let mut entries = self.entries.write().await;
        entries.clear();
        *self.hits.write().await = 0;
        *self.misses.write().await = 0;
    }

    /// Perform cache maintenance: remove expired entries and handle over-capacity
    pub async fn cleanup(&self) {
        let mut entries = self.entries.write().await;
        self.cleanup_expired_entries(&mut entries).await;
        if entries.len() >= self.config.max_size {
            self.evict_entries(&mut entries).await;
        }
    }

    /// Get cache statistics
    pub async fn stats(&self) -> CacheStats {
        let entries = self.entries.read().await;
        let hits = *self.hits.read().await;
        let misses = *self.misses.read().await;
        let total_requests = hits + misses;

        let hit_rate = if total_requests > 0 {
            (hits as f64) / (total_requests as f64)
        } else {
            0.0
        };

        CacheStats {
            size: entries.len(),
            max_size: self.config.max_size,
            hits,
            misses,
            hit_rate,
            ttl_default: self.config.default_ttl,
        }
    }

    /// Remove expired entries from the cache
    async fn cleanup_expired_entries(&self, entries: &mut HashMap<K, CacheEntry<V>>) {
        let initial_len = entries.len();
        entries.retain(|_, entry| !entry.is_expired());
        let removed_count = initial_len - entries.len();

        if removed_count > 0 {
            info!("Cleaned up {} expired cache entries", removed_count);
        }
    }

    /// Evict entries when cache is over capacity
    async fn evict_entries(&self, entries: &mut HashMap<K, CacheEntry<V>>) {
        // Convert to vector of (key, age) pairs and sort by age (oldest first)
        let mut aged_keys: Vec<(K, Duration)> = entries
            .iter()
            .map(|(k, entry)| {
                let age = Instant::now() - entry.created_at;
                (k.clone(), age)
            })
            .collect();

        aged_keys.sort_by(|a, b| a.1.cmp(&b.1));  // Sort by age, oldest first

        // Remove the oldest entries up to batch size
        let to_remove = std::cmp::min(self.config.eviction_batch_size, aged_keys.len());
        for i in 0..to_remove {
            let (key, _) = &aged_keys[i];
            entries.remove(key);
        }

        info!("Evicted {} entries from cache due to capacity limit", to_remove);
    }

    /// Check if cache is at max capacity
    pub async fn is_full(&self) -> bool {
        let entries = self.entries.read().await;
        entries.len() >= self.config.max_size
    }
}

/// Cache statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub size: usize,
    pub max_size: usize,
    pub hits: u64,
    pub misses: u64,
    pub hit_rate: f64,
    pub ttl_default: Duration,
}

/// Specific cache for AI model responses
pub type AiResponseCache = AiCache<String, serde_json::Value>;

impl AiResponseCache {
    /// Create a cache optimized for AI model responses
    pub fn ai_response_cache() -> Self {
        let config = CacheConfig {
            default_ttl: Duration::from_secs(600),  // 10 minutes for AI responses
            max_size: 5000,                        // Larger for AI responses
            eviction_batch_size: 500,              // Evict more at a time
            cleanup_interval: Duration::from_secs(300), // Clean every 5 minutes
            enable_compression: true,              // Enable compression for large responses
        };
        Self::new(config)
    }

    /// Cache an AI model response
    pub async fn cache_ai_response(
        &self,
        model_name: &str,
        input_hash: &str,
        response: serde_json::Value,
        ttl: Option<Duration>,
    ) -> Result<(), String> {
        // Create a composite key that includes both model and input hash
        let key = format!("{}:{}", model_name, input_hash);
        self.put(key, response, ttl).await
    }

    /// Retrieve a cached AI model response
    pub async fn get_cached_response(
        &self,
        model_name: &str,
        input_hash: &str,
    ) -> Option<serde_json::Value> {
        let key = format!("{}:{}", model_name, input_hash);
        self.get(&key).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_cache_put_get() {
        let cache = AiCache::new(CacheConfig::default());
        let key = "test_key";
        let value = "test_value";

        cache.put(key.to_string(), value.to_string(), None).await.unwrap();
        let retrieved = cache.get(&key.to_string()).await;

        assert_eq!(retrieved, Some(value.to_string()));
    }

    #[tokio::test]
    async fn test_cache_expiry() {
        let config = CacheConfig {
            default_ttl: Duration::from_millis(10),
            ..CacheConfig::default()
        };
        let cache = AiCache::new(config);

        cache
            .put("expiring_key".to_string(), "value".to_string(), None)
            .await
            .unwrap();

        // Wait for expiry
        tokio::time::sleep(Duration::from_millis(15)).await;

        let result = cache.get(&"expiring_key".to_string()).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_cache_stats() {
        let cache = AiCache::new(CacheConfig::default());

        cache.put("key1".to_string(), "value1".to_string(), None).await.unwrap();
        let _ = cache.get(&"key1".to_string()).await; // Hit
        let _ = cache.get(&"key2".to_string()).await; // Miss

        let stats = cache.stats().await;
        assert_eq!(stats.size, 1);
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.hit_rate, 0.5);
    }

    #[tokio::test]
    async fn test_ai_response_cache() {
        let cache = AiResponseCache::ai_response_cache();
        let model = "gpt-4";
        let input_hash = "abc123";
        let response = serde_json::json!({"choices": [{"message": {"content": "Hello"}}]});

        cache.cache_ai_response(model, input_hash, response.clone(), None).await.unwrap();
        let cached = cache.get_cached_response(model, input_hash).await;

        assert_eq!(cached, Some(response));
    }
}