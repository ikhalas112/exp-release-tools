//! LRU Cache module for storing decrypted chunks
//!
//! Provides an efficient Least Recently Used cache to store
//! decrypted data chunks and avoid redundant decryption operations.

use std::collections::{HashMap, VecDeque};
use std::hash::Hash;
use std::num::NonZeroUsize;

/// LRU Cache for storing decrypted chunks
///
/// Uses a HashMap for O(1) lookups and a VecDeque to track
/// access order for LRU eviction.
#[derive(Debug)]
pub struct LruCache<K, V> {
    /// Maximum number of entries in the cache
    capacity: NonZeroUsize,

    /// Cache entries mapping keys to values
    entries: HashMap<K, V>,

    /// Access order tracking (front = most recent, back = least recent)
    access_order: VecDeque<K>,
}

impl<K, V> LruCache<K, V>
where
    K: Eq + Hash + Clone,
{
    /// Create a new LRU cache with the specified capacity
    ///
    /// # Arguments
    ///
    /// * `capacity` - Maximum number of entries (must be > 0)
    ///
    /// # Panics
    ///
    /// Panics if capacity is 0
    pub fn new(capacity: usize) -> Self {
        let capacity =
            NonZeroUsize::new(capacity).expect("LRU cache capacity must be greater than 0");

        Self {
            capacity,
            entries: HashMap::new(),
            access_order: VecDeque::new(),
        }
    }

    /// Insert or update a key-value pair in the cache
    ///
    /// If the key already exists, its value is updated and it's moved
    /// to the most recently used position. If the cache is full,
    /// the least recently used entry is evicted.
    ///
    /// # Arguments
    ///
    /// * `key` - The cache key
    /// * `value` - The value to cache
    pub fn insert(&mut self, key: K, value: V) {
        // Update access order
        if let Some(pos) = self.access_order.iter().position(|k| k == &key) {
            self.access_order.remove(pos);
        }
        self.access_order.push_front(key.clone());

        // Insert/update entry
        self.entries.insert(key, value);

        // Evict if over capacity
        while self.entries.len() > self.capacity.get() {
            if let Some(lru_key) = self.access_order.pop_back() {
                self.entries.remove(&lru_key);
            }
        }
    }

    /// Get a value from the cache by key
    ///
    /// Returns `None` if the key is not in the cache.
    /// If the key exists, it's moved to the most recently used position.
    ///
    /// # Arguments
    ///
    /// * `key` - The cache key to look up
    ///
    /// # Returns
    ///
    /// `Some(&V)` if the key exists, `None` otherwise
    pub fn get(&mut self, key: &K) -> Option<&V> {
        if self.entries.contains_key(key) {
            // Move to most recent position
            if let Some(pos) = self.access_order.iter().position(|k| k == key) {
                let key_clone = self.access_order[pos].clone();
                self.access_order.remove(pos);
                self.access_order.push_front(key_clone);
            }
            self.entries.get(key)
        } else {
            None
        }
    }

    /// Remove a specific entry from the cache
    ///
    /// # Arguments
    ///
    /// * `key` - The cache key to remove
    ///
    /// # Returns
    ///
    /// `Some(V)` if the key existed, `None` otherwise
    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.access_order.retain(|k| k != key);
        self.entries.remove(key)
    }

    /// Clear all entries from the cache
    pub fn clear(&mut self) {
        self.entries.clear();
        self.access_order.clear();
    }

    /// Get the current number of entries in the cache
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the cache is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get the maximum capacity of the cache
    pub fn capacity(&self) -> usize {
        self.capacity.get()
    }

    /// Check if the cache is at capacity
    pub fn is_full(&self) -> bool {
        self.len() >= self.capacity()
    }
}

impl<K, V> Clone for LruCache<K, V>
where
    K: Eq + Hash + Clone,
    V: Clone,
{
    fn clone(&self) -> Self {
        Self {
            capacity: self.capacity,
            entries: self.entries.clone(),
            access_order: self.access_order.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lru_cache_creation() {
        let cache: LruCache<u32, String> = LruCache::new(3);
        assert_eq!(cache.capacity(), 3);
        assert!(cache.is_empty());
    }

    #[test]
    fn test_lru_cache_insert_get() {
        let mut cache = LruCache::new(3);
        cache.insert(1, "one".to_string());
        cache.insert(2, "two".to_string());

        assert_eq!(cache.get(&1), Some(&"one".to_string()));
        assert_eq!(cache.get(&2), Some(&"two".to_string()));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn test_lru_cache_update() {
        let mut cache = LruCache::new(3);
        cache.insert(1, "one".to_string());
        cache.insert(1, "ONE".to_string());

        assert_eq!(cache.get(&1), Some(&"ONE".to_string()));
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_lru_cache_eviction() {
        let mut cache = LruCache::new(2);
        cache.insert(1, "one".to_string());
        cache.insert(2, "two".to_string());

        // Access key 1 to make it most recent
        cache.get(&1);

        // Insert third item should evict key 2 (least recently used)
        cache.insert(3, "three".to_string());

        assert_eq!(cache.get(&1), Some(&"one".to_string()));
        assert_eq!(cache.get(&2), None); // Evicted
        assert_eq!(cache.get(&3), Some(&"three".to_string()));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn test_lru_cache_remove() {
        let mut cache = LruCache::new(3);
        cache.insert(1, "one".to_string());
        cache.insert(2, "two".to_string());

        let removed = cache.remove(&1);
        assert_eq!(removed, Some("one".to_string()));
        assert_eq!(cache.get(&1), None);
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_lru_cache_clear() {
        let mut cache = LruCache::new(3);
        cache.insert(1, "one".to_string());
        cache.insert(2, "two".to_string());

        cache.clear();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_lru_cache_is_full() {
        let mut cache: LruCache<u32, u32> = LruCache::new(2);
        assert!(!cache.is_full());

        cache.insert(1, 1);
        assert!(!cache.is_full());

        cache.insert(2, 2);
        assert!(cache.is_full());

        cache.insert(3, 3); // Evicts one
        assert!(cache.is_full());
    }

    #[test]
    fn test_lru_cache_access_order() {
        let mut cache = LruCache::new(3);
        cache.insert(1, 1);
        cache.insert(2, 2);
        cache.insert(3, 3);

        // Access key 1 (should become most recent)
        cache.get(&1);

        // Insert key 4 (should evict key 2, now least recent)
        cache.insert(4, 4);

        assert_eq!(cache.get(&1), Some(&1));
        assert_eq!(cache.get(&2), None); // Evicted
        assert_eq!(cache.get(&3), Some(&3));
        assert_eq!(cache.get(&4), Some(&4));
    }

    #[test]
    fn test_lru_cache_clone() {
        let mut cache1 = LruCache::new(2);
        cache1.insert(1, "one".to_string());
        cache1.insert(2, "two".to_string());

        let mut cache2 = cache1.clone();

        assert_eq!(cache2.get(&1), Some(&"one".to_string()));
        assert_eq!(cache2.get(&2), Some(&"two".to_string()));
        assert_eq!(cache2.len(), 2);
    }

    #[test]
    fn test_lru_cache_string_keys() {
        let mut cache: LruCache<String, Vec<u8>> = LruCache::new(3);

        cache.insert("file1.bin".to_string(), vec![1, 2, 3]);
        cache.insert("file2.bin".to_string(), vec![4, 5, 6]);

        assert_eq!(cache.get(&"file1.bin".to_string()), Some(&vec![1, 2, 3]));
        assert_eq!(cache.get(&"file2.bin".to_string()), Some(&vec![4, 5, 6]));
        assert_eq!(cache.get(&"file3.bin".to_string()), None);
    }

    #[test]
    #[should_panic(expected = "LRU cache capacity must be greater than 0")]
    fn test_lru_cache_zero_capacity_panics() {
        let _cache: LruCache<u32, u32> = LruCache::new(0);
    }
}
