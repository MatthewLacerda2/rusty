//! A cached load of a scene's textures peaks no higher than the same textures
//! decoded from their PNGs (#2372); the fixture and measure: `textures_gpu_tests`.

use super::textures_gpu_tests::{compresses, images, load, mb, set_up, N};
use crate::memory::rss_peak_bytes;

#[test]
fn gpu_cached_textures_peak_no_higher_than_the_same_decoded() {
    rusty_test_support::alone!(|| {
        let Some((mut renderer, dir)) = set_up("cached_or_decoded") else {
            return;
        };
        let cached = images(&dir, "cached", 4 * N + 1, true);
        let decoded = images(&dir, "decoded", 4 * N, false);
        if !compresses(&mut renderer, &cached) {
            return;
        }
        let start = rss_peak_bytes().expect("read");
        // The cached load first: the decoded one may reuse what it freed, never
        // the other way round.
        let (after_cached, _) = load(&mut renderer, &cached[1..]);
        let (after_decoded, _) = load(&mut renderer, &decoded);
        let (cached_b, decoded_b) = (after_cached - start, after_decoded - after_cached);
        println!(
            "{} textures: cached +{} MB, decoded +{} MB",
            4 * N,
            mb(cached_b),
            mb(decoded_b)
        );
        assert!(
            cached_b <= decoded_b,
            "{} textures loaded from their caches peaked {} MB higher, the same decoded \
             from their PNGs {} MB: a cached load holds more than a decode",
            4 * N,
            mb(cached_b),
            mb(decoded_b)
        );
    });
}
