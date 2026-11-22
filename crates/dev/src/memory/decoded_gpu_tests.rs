//! A direction check only: on CI's adapters it does not fail for an unbounded cached
//! upload, which `textures_gpu_tests`' four-times test catches.

use super::textures_gpu_tests::{compresses, images, load, set_up, N};
use crate::memory::rss_peak_bytes;
use rusty_render::STAGING_BUDGET;

/// Bytes a cached load may hold past a decoded one: its one staging buffer in flight.
/// Measured, cached then decoded: lavapipe 34 then 294 MB, Metal 17 then 34 MB, once
/// 16 and 16 MB.
const SLACK: u64 = STAGING_BUDGET;

#[test]
fn gpu_cached_textures_peak_at_most_a_staging_budget_past_the_same_decoded() {
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
            "{} textures, beside their GPU copies: cached +{cached_b} B ({cached_gpu} B on the \
             GPU), decoded +{decoded_b} B ({decoded_gpu} B on the GPU)",
            4 * N
        );
        assert!(
            cached_b <= decoded_b + SLACK,
            "{} textures loaded from their caches held {cached_b} B past their GPU copies, \
             the same decoded from their PNGs {decoded_b} B, more than {SLACK} B of slack \
             apart: a cached load holds more beside the GPU than a decode (staging \
             unbounded, or a CPU copy kept)",
            4 * N
        );
    });
}
