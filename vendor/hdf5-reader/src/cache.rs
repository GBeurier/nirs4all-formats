use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::Arc;

use lru::LruCache;
use parking_lot::{Condvar, Mutex};
use smallvec::SmallVec;

use crate::error::Result;

/// Key for the chunk cache: (dataset object header address, chunk offset tuple).
///
/// Uses `SmallVec<[u64; 4]>` to avoid heap allocation for datasets with up to
/// 4 dimensions (the common case for climate/science data).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ChunkKey {
    pub dataset_addr: u64,
    pub chunk_offsets: SmallVec<[u64; 4]>,
}

/// LRU cache for decompressed chunks.
///
/// Thread-safe via `parking_lot::Mutex` (non-poisoning). Values are
/// `Arc<Vec<u8>>` so multiple readers can share the same decompressed chunk data.
pub struct ChunkCache {
    inner: Mutex<ChunkCacheState>,
    max_bytes: usize,
    max_slots: usize,
}

struct ChunkCacheState {
    cache: LruCache<ChunkKey, Arc<Vec<u8>>>,
    current_bytes: usize,
    in_flight: HashMap<ChunkKey, Arc<InFlightLoad>>,
    hits: u64,
    misses: u64,
    inserts: u64,
    evictions: u64,
}

struct InFlightLoad {
    completed: Mutex<bool>,
    ready: Condvar,
}

/// Point-in-time statistics for a [`ChunkCache`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ChunkCacheStats {
    pub hits: u64,
    pub misses: u64,
    pub inserts: u64,
    pub evictions: u64,
    pub current_bytes: usize,
    pub entries: usize,
    pub in_flight: usize,
    pub max_bytes: usize,
    pub max_slots: usize,
}

impl ChunkCache {
    /// Create a new chunk cache.
    ///
    /// - `max_bytes`: maximum total bytes of decompressed data to cache (default 64 MiB)
    /// - `max_slots`: maximum number of entries (default 521)
    pub fn new(max_bytes: usize, max_slots: usize) -> Self {
        let slots = NonZeroUsize::new(max_slots).unwrap_or(NonZeroUsize::new(521).unwrap());
        ChunkCache {
            inner: Mutex::new(ChunkCacheState {
                cache: LruCache::new(slots),
                current_bytes: 0,
                in_flight: HashMap::new(),
                hits: 0,
                misses: 0,
                inserts: 0,
                evictions: 0,
            }),
            max_bytes,
            max_slots: slots.get(),
        }
    }

    /// Get a cached chunk, if present. Promotes the entry in LRU order.
    pub fn get(&self, key: &ChunkKey) -> Option<Arc<Vec<u8>>> {
        let mut cache = self.inner.lock();
        let result = cache.cache.get(key).cloned();
        if result.is_some() {
            cache.hits += 1;
        } else {
            cache.misses += 1;
        }
        result
    }

    /// Insert a chunk into the cache. Evicts LRU entries if over capacity.
    pub fn insert(&self, key: ChunkKey, data: Vec<u8>) -> Arc<Vec<u8>> {
        let data_len = data.len();
        let arc = Arc::new(data);

        if self.max_bytes == 0 || data_len > self.max_bytes {
            return arc;
        }

        let mut state = self.inner.lock();
        // Evict until we have room
        while state.current_bytes + data_len > self.max_bytes && !state.cache.is_empty() {
            if let Some((_, evicted)) = state.cache.pop_lru() {
                state.current_bytes = state.current_bytes.saturating_sub(evicted.len());
                state.evictions += 1;
            }
        }

        if let Some(replaced) = state.cache.peek(&key) {
            state.current_bytes = state.current_bytes.saturating_sub(replaced.len());
        }
        state.current_bytes += data_len;
        state.inserts += 1;
        state.cache.put(key, arc.clone());

        arc
    }

    /// Return a cached chunk or compute it once across concurrent callers.
    pub fn get_or_insert_with<F>(&self, key: ChunkKey, load: F) -> Result<Arc<Vec<u8>>>
    where
        F: FnOnce() -> Result<Vec<u8>>,
    {
        loop {
            let in_flight = {
                let mut state = self.inner.lock();
                if let Some(cached) = state.cache.get(&key).cloned() {
                    state.hits += 1;
                    return Ok(cached);
                }
                state.misses += 1;

                if let Some(in_flight) = state.in_flight.get(&key) {
                    Arc::clone(in_flight)
                } else {
                    let in_flight = Arc::new(InFlightLoad {
                        completed: Mutex::new(false),
                        ready: Condvar::new(),
                    });
                    state.in_flight.insert(key.clone(), Arc::clone(&in_flight));
                    drop(state);

                    let result = load().map(|data| self.insert(key.clone(), data));

                    let mut state = self.inner.lock();
                    state.in_flight.remove(&key);
                    let mut completed = in_flight.completed.lock();
                    *completed = true;
                    in_flight.ready.notify_all();

                    return result;
                }
            };

            let mut completed = in_flight.completed.lock();
            while !*completed {
                in_flight.ready.wait(&mut completed);
            }
        }
    }

    /// Return current cache statistics.
    pub fn stats(&self) -> ChunkCacheStats {
        let state = self.inner.lock();
        ChunkCacheStats {
            hits: state.hits,
            misses: state.misses,
            inserts: state.inserts,
            evictions: state.evictions,
            current_bytes: state.current_bytes,
            entries: state.cache.len(),
            in_flight: state.in_flight.len(),
            max_bytes: self.max_bytes,
            max_slots: self.max_slots,
        }
    }
}

impl Default for ChunkCache {
    fn default() -> Self {
        Self::new(64 * 1024 * 1024, 521)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_insert_and_get() {
        let cache = ChunkCache::new(1024, 10);
        let key = ChunkKey {
            dataset_addr: 100,
            chunk_offsets: SmallVec::from_vec(vec![0, 0]),
        };
        cache.insert(key.clone(), vec![1, 2, 3]);
        let val = cache.get(&key).unwrap();
        assert_eq!(&**val, &[1, 2, 3]);
    }

    #[test]
    fn cache_eviction() {
        let cache = ChunkCache::new(10, 10); // 10 bytes max
        for i in 0..5 {
            let key = ChunkKey {
                dataset_addr: 100,
                chunk_offsets: SmallVec::from_vec(vec![i]),
            };
            cache.insert(key, vec![0; 4]); // 4 bytes each
        }
        // Should have evicted older entries to stay under 10 bytes
        // At most 2 entries of 4 bytes each = 8 bytes
        let first_key = ChunkKey {
            dataset_addr: 100,
            chunk_offsets: SmallVec::from_vec(vec![0]),
        };
        assert!(cache.get(&first_key).is_none()); // should be evicted
    }

    #[test]
    fn cache_disabled_bypasses_storage() {
        let cache = ChunkCache::new(0, 10);
        let key = ChunkKey {
            dataset_addr: 100,
            chunk_offsets: SmallVec::from_vec(vec![0]),
        };
        cache.insert(key.clone(), vec![1, 2, 3]);
        assert!(cache.get(&key).is_none());
    }

    #[test]
    fn cache_promotes_on_get() {
        // Verify that get() promotes entries in LRU order (the bug fix).
        let cache = ChunkCache::new(12, 10); // room for 3 entries of 4 bytes
        let key_a = ChunkKey {
            dataset_addr: 1,
            chunk_offsets: SmallVec::from_vec(vec![0]),
        };
        let key_b = ChunkKey {
            dataset_addr: 2,
            chunk_offsets: SmallVec::from_vec(vec![0]),
        };
        let key_c = ChunkKey {
            dataset_addr: 3,
            chunk_offsets: SmallVec::from_vec(vec![0]),
        };

        cache.insert(key_a.clone(), vec![0; 4]); // LRU order: a
        cache.insert(key_b.clone(), vec![0; 4]); // LRU order: a, b
        cache.insert(key_c.clone(), vec![0; 4]); // LRU order: a, b, c

        // Access key_a to promote it
        assert!(cache.get(&key_a).is_some()); // LRU order: b, c, a

        // Insert a new entry that forces eviction
        let key_d = ChunkKey {
            dataset_addr: 4,
            chunk_offsets: SmallVec::from_vec(vec![0]),
        };
        cache.insert(key_d, vec![0; 4]); // Should evict b (LRU)

        assert!(cache.get(&key_a).is_some()); // a was promoted, should survive
        assert!(cache.get(&key_b).is_none()); // b was LRU, should be evicted
    }

    #[test]
    fn cache_replacement_updates_accounting() {
        let cache = ChunkCache::new(8, 10);
        let key = ChunkKey {
            dataset_addr: 100,
            chunk_offsets: SmallVec::from_vec(vec![0]),
        };

        cache.insert(key.clone(), vec![1, 2, 3, 4]);
        cache.insert(key.clone(), vec![5, 6]);

        let other = ChunkKey {
            dataset_addr: 100,
            chunk_offsets: SmallVec::from_vec(vec![1]),
        };
        cache.insert(other.clone(), vec![7, 8, 9, 10]);

        assert_eq!(&**cache.get(&key).unwrap(), &[5, 6]);
        assert!(cache.get(&other).is_some());
    }

    #[test]
    fn cache_get_or_insert_with_deduplicates_concurrent_loads() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let cache = Arc::new(ChunkCache::new(1024, 10));
        let key = ChunkKey {
            dataset_addr: 100,
            chunk_offsets: SmallVec::from_vec(vec![0, 0]),
        };
        let load_count = Arc::new(AtomicUsize::new(0));

        std::thread::scope(|scope| {
            for _ in 0..8 {
                let cache = Arc::clone(&cache);
                let key = key.clone();
                let load_count = Arc::clone(&load_count);
                scope.spawn(move || {
                    let value = cache
                        .get_or_insert_with(key, || {
                            load_count.fetch_add(1, Ordering::SeqCst);
                            std::thread::sleep(std::time::Duration::from_millis(10));
                            Ok(vec![1, 2, 3, 4])
                        })
                        .unwrap();
                    assert_eq!(&*value, &[1, 2, 3, 4]);
                });
            }
        });

        assert_eq!(load_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn cache_stats_track_hits_misses_and_evictions() {
        let cache = ChunkCache::new(8, 2);
        let key_a = ChunkKey {
            dataset_addr: 1,
            chunk_offsets: SmallVec::from_vec(vec![0]),
        };
        let key_b = ChunkKey {
            dataset_addr: 1,
            chunk_offsets: SmallVec::from_vec(vec![1]),
        };
        let key_c = ChunkKey {
            dataset_addr: 1,
            chunk_offsets: SmallVec::from_vec(vec![2]),
        };

        assert!(cache.get(&key_a).is_none());
        cache.insert(key_a.clone(), vec![1, 2, 3, 4]);
        assert!(cache.get(&key_a).is_some());
        cache.insert(key_b, vec![5, 6, 7, 8]);
        cache.insert(key_c, vec![9, 10, 11, 12]);

        let stats = cache.stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.inserts, 3);
        assert_eq!(stats.evictions, 1);
        assert_eq!(stats.entries, 2);
        assert_eq!(stats.current_bytes, 8);
        assert_eq!(stats.max_bytes, 8);
        assert_eq!(stats.max_slots, 2);
    }
}
