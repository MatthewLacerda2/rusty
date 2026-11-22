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
    let whole = off.triangles;
    assert_eq!(first.hiz_depth_triangles, whole, "{first:?}");
    let passes = 2 + u64::from(first.primed_passes);
    assert_eq!(first.triangles, passes * whole, "{off:?} {first:?}");
}
