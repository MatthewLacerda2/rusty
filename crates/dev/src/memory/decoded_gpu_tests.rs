//! A direction check: a cached load of a scene's textures holds no more beside
//! its GPU copies than the same textures decoded from their PNGs. The fixture
//! and measure: `textures_gpu_tests`.
//!
//! It cannot single out the cached path's staging bound on the adapters CI runs,
//! so it does not fail when that bound is switched off. With mips, each decoded
//! texture keeps resident memory the device's report leaves out (the mip pass's
//! render-attachment memory), more than the unbounded cached staging adds; without
//! mips, the decoded side's single-level writes go through the same bound, so
//! switching it off raises both sides. The test that fails for an unbounded cached
//! upload is `textures_gpu_tests::gpu_four_times_the_cached_textures_peak_higher_only_by_their_gpu_copies`.

use super::textures_gpu_tests::{compresses, images, load, mb, set_up, N};
use crate::memory::rss_peak_bytes;

/// A cached load never peaks above the same textures decoded, GPU copies aside.
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
        let (after_cached, cached_gpu) = load(&mut renderer, &cached[1..]);
        let (after_decoded, decoded_gpu) = load(&mut renderer, &decoded);
        // Each side's growth less what its GPU copies hold (the decoded ones are
        // RGBA8, four times the blocks): what the load held beside them, its
        // staging and any CPU copy, which is what the cache must not raise.
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
