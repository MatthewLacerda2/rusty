//! Stable across Rust releases and platforms, which std's `DefaultHasher` is not. [`content`]
//! (xxh3) is far faster than [`fnv1a64`] in the opt-level-0 dev build, but fnv1a64 stays
//! where a stored digest is compared with a fresh one; changing it breaks what they hold.

pub fn content(bytes: &[u8]) -> u64 {
    xxhash_rust::xxh3::xxh3_64(bytes)
}

#[derive(Default)]
pub struct Content(xxhash_rust::xxh3::Xxh3);

impl Content {
    pub fn write(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }

    pub fn finish(&self) -> u64 {
        self.0.digest()
    }
}

/// FNV-1a, 64-bit. Only for digests already stored (the `.seeded` manifest,
/// `fingerprints.txt`); new uses take [`content`].
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
    fn content_in_pieces_is_content_of_them_joined() {
        let mut pieces = super::Content::default();
        pieces.write(b"occlu");
        pieces.write(b"sion");
        assert_eq!(pieces.finish(), super::content(b"occlusion"));
    }

    #[test]
    fn matches_the_published_fnv1a_64_vectors() {
        assert_eq!(super::fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(super::fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
    }
}
