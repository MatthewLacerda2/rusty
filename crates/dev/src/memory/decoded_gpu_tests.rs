//! A direction check only: on CI's adapters it does not fail for an unbounded cached
//! upload, which `textures_gpu_tests`' four-times test catches.

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
        // Cached first: the decoded load may reuse what it freed, never the other way round.
        let (after_cached, cached_gpu) = load(&mut renderer, &cached[1..]);
        let (after_decoded, decoded_gpu) = load(&mut renderer, &decoded);
        let beside = |grew: u64, gpu: u64| grew.saturating_sub(gpu);
        let cached_b = beside(after_cached - start, cached_gpu);
        let decoded_b = beside(after_decoded - after_cached, decoded_gpu);
        println!(
            "{} textures, beside their GPU copies: cached +{} MB ({} MB on the GPU), \
             decoded +{} MB ({} MB on the GPU)",
            4 * N,
            mb(cached_b),
            mb(cached_gpu),
            mb(decoded_b),
            mb(decoded_gpu)
        );
        assert!(
            cached_b <= decoded_b,
            "{} textures loaded from their caches peaked {} MB past their GPU copies, \
             the same decoded from their PNGs {} MB: a cached load holds more beside \
             the GPU than a decode (staging unbounded, or a CPU copy kept)",
            4 * N,
            mb(cached_b),
            mb(decoded_b)
        );
    });
}
