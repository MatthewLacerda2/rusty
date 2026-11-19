//! How many compute passes the `occlusion` stage records (#2327): the select's
//! own, before phase one's depth draw, then one for the pyramid, the test, the
//! compaction and every clustered batch's cluster cull, however many batches.

use glam::{Quat, Vec3};

use super::cluster_gpu_tests::panel;
use super::gpu_tests::{offscreen, shot, H, W};
use rusty_lighting::occlusion::test_level::block;
use rusty_scene::{Camera, Scene};

/// The stage's compute passes with any number of clustered batches.
/// Measured: lavapipe 2 (7 before #2327, with these three batches: 4 + R).
const PASSES: u32 = 2;

/// A wall half across three clustered panels, each its own mesh and so its own
/// clustered batch.
fn three_panels() -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let (lo, hi) = (Vec3::new(-9.0, -3.0, -0.2), Vec3::new(0.0, 12.0, 0.2));
    block(&mut scene, "Wall", lo, hi);
    for (n, x) in [-7.0, -2.0, 3.0].into_iter().enumerate() {
        let at = Vec3::new(x, -2.0, -5.0 - n as f32);
        panel(&mut scene, &format!("Panel_{n}"), (at, 4.0, Quat::IDENTITY));
    }
    scene
}

#[test]
fn gpu_the_stage_records_two_compute_passes_whatever_its_clustered_batches() {
    let Some(mut r) = crate::test_gpu::culling::culling_or_skip(W, H) else {
        return;
    };
    let scene = three_panels();
    let cam = Camera::new(Vec3::new(0.0, 1.0, 8.0), -90.0, 0.0);
    let mut view = offscreen(&r);
    // A few frames, so the history and the counters' read back settle.
    let shots: Vec<_> = (0..9)
        .map(|_| shot(&mut r, &mut view, &scene, &cam))
        .collect();
    let (on, c) = shots.last().expect("nine frames");
    assert_eq!(c.ops.compute_passes, PASSES, "{c:?}");
    // What is tested and drawn is decided on the CPU, so it is the same anywhere.
    // Measured: lavapipe 4/258/4/24588 (the same before #2327; it culled 1 and 122).
    let decided = (c.hiz_tested, c.cluster_tested, c.draw_calls, c.triangles);
    assert_eq!(decided, (4, 258, 4, 24588), "{c:?}");
    assert!(c.hiz_culled > 0 && c.cluster_culled > 0, "{c:?}");
    r.quality.gpu_occlusion_culling = false;
    let mut fresh = offscreen(&r);
    let (off, _) = shot(&mut r, &mut fresh, &scene, &cam);
    assert!(*on == off, "the cull moved pixels");
}
