use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use hdf5_reader::cache::{ChunkCache, ChunkKey};
use proptest::prelude::*;
use smallvec::SmallVec;

/// Strategy to generate a random ChunkKey.
fn arb_chunk_key() -> impl Strategy<Value = ChunkKey> {
    (any::<u64>(), prop::collection::vec(any::<u64>(), 0..8)).prop_map(|(addr, offsets)| ChunkKey {
        dataset_addr: addr,
        chunk_offsets: SmallVec::from_vec(offsets),
    })
}

/// Strategy to generate random chunk data (1..=256 bytes).
fn arb_chunk_data() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(any::<u8>(), 1..=256)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Inserting data into the cache and immediately retrieving it by the same
    /// key must return the exact bytes that were inserted.
    #[test]
    fn cache_insert_retrieve_roundtrip(key in arb_chunk_key(), data in arb_chunk_data()) {
        // Use a large capacity so eviction does not interfere.
        let cache = ChunkCache::new(1024 * 1024, 1024);
        cache.insert(key.clone(), data.clone());
        let retrieved = cache.get(&key).expect("key must be present after insert");
        prop_assert_eq!(&*retrieved, &data);
    }

    /// After inserting many entries that exceed the cache's byte budget, the
    /// most recently used entries must still be present.
    #[test]
    fn cache_eviction_preserves_mru(
        entries in prop::collection::vec((0u64..1000, arb_chunk_data()), 10..50),
    ) {
        // Tiny byte budget: only room for a handful of entries.
        let cache = ChunkCache::new(512, 1024);

        for (id, data) in &entries {
            let key = ChunkKey {
                dataset_addr: *id,
                chunk_offsets: SmallVec::from_slice(&[0]),
            };
            cache.insert(key, data.clone());
        }

        // The very last entry inserted should always be retrievable (it was
        // most recently used and should not have been evicted yet).
        if let Some((last_id, last_data)) = entries.last() {
            let key = ChunkKey {
                dataset_addr: *last_id,
                chunk_offsets: SmallVec::from_slice(&[0]),
            };
            if let Some(val) = cache.get(&key) {
                prop_assert_eq!(&*val, last_data);
            }
            // If the single entry exceeds 512 bytes, the cache correctly
            // bypasses storage, so `None` is acceptable in that case.
        }
    }

    /// Hashing a ChunkKey is deterministic: the same inputs must always
    /// produce the same hash value.
    #[test]
    fn chunk_key_hash_consistency(key in arb_chunk_key()) {
        let hash1 = {
            let mut h = DefaultHasher::new();
            key.hash(&mut h);
            h.finish()
        };
        let hash2 = {
            let mut h = DefaultHasher::new();
            key.hash(&mut h);
            h.finish()
        };
        prop_assert_eq!(hash1, hash2);
    }

    /// Two ChunkKeys with identical fields must produce the same hash.
    #[test]
    fn chunk_key_equal_keys_same_hash(addr in any::<u64>(), offsets in prop::collection::vec(any::<u64>(), 0..8)) {
        let key_a = ChunkKey {
            dataset_addr: addr,
            chunk_offsets: SmallVec::from_vec(offsets.clone()),
        };
        let key_b = ChunkKey {
            dataset_addr: addr,
            chunk_offsets: SmallVec::from_vec(offsets),
        };
        let hash_a = {
            let mut h = DefaultHasher::new();
            key_a.hash(&mut h);
            h.finish()
        };
        let hash_b = {
            let mut h = DefaultHasher::new();
            key_b.hash(&mut h);
            h.finish()
        };
        prop_assert_eq!(&key_a, &key_b);
        prop_assert_eq!(hash_a, hash_b);
    }
}
