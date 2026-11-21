//! A culled camera's first frame, before any read back of what its passes drew
//! has come back (#2387): each counts the CPU's triangles, every instance, an
//! upper bound, never `0`.

use super::draws_gpu_tests::{crates, frames, scene, settled};
use super::gpu_tests::{H, W};
#[test]
fn gpu_a_frame_with_no_read_back_yet_counts_the_cpu_s_triangles() {
    let mut s = scene(false);
    crates(&mut s);
    let Some(mut r) = crate::test_gpu::culling::culling_or_skip(W, H) else {
        return;
    };
    let (_, off) = settled(&mut r, &s, false);
    let (_, first) = frames(&mut r, &s, true, 1).pop().expect("one frame");
    assert!(first.hiz_tested > 0, "the first frame culls: {first:?}");
    // Each culled pass counts every instance: phase one, the late set where the
    // cull primes, the forward pass.
    let whole = off.triangles;
    assert_eq!(first.hiz_depth_triangles, whole, "{first:?}");
    let passes = 2 + u64::from(first.primed_passes);
    assert_eq!(first.triangles, passes * whole, "{off:?} {first:?}");
}
