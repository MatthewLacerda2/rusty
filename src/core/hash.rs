//! src/core/hash.rs — hashes stable across Rust releases and platforms (#939), which
//! std's `DefaultHasher` is not.
//!
//! [`content`] fingerprints what the engine reads or writes: import-cache sources and
//! keys, texture caches, lightmap file tags, demo digests. It is xxh3 (#1326): compiled
//! at opt-level 2 as a dependency, it runs about 40 times faster than [`fnv1a64`] does
//! in rusty's own opt-level-0 dev build, which the editor runs on every launch.
//! [`fnv1a64`] stays only where a digest is already stored in game repos (the scene
//! manifest's `.seeded` records), so changing it would break what they hold.

/// xxh3, 64-bit, seed 0: the content hash.
pub fn content(bytes: &[u8]) -> u64 {
    xxhash_rust::xxh3::xxh3_64(bytes)
}

/// FNV-1a, 64-bit. Only for digests already committed in game repos; new uses take
/// [`content`].
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn content_matches_the_published_xxh3_64_vector() {
        // XXH3_64bits("") from the xxHash reference implementation.
        assert_eq!(super::content(b""), 0x2d06_8005_38d3_94c2);
        assert_ne!(super::content(b"a"), super::content(b"b"));
    }

    #[test]
    fn matches_the_published_fnv1a_64_vectors() {
        assert_eq!(super::fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(super::fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
    }
}
