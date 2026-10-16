//! src/core/hash.rs — a content hash stable across Rust releases and platforms (#939),
//! which std's `DefaultHasher` is not. Used to tag file contents.

/// FNV-1a, 64-bit.
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn matches_the_published_fnv1a_64_vectors() {
        assert_eq!(super::fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(super::fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
    }
}
