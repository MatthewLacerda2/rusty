//! GPU occlusion culling on the Dust2-sized bench level (#1963), measured by hand:
//! `cargo nextest run --features dev --run-ignored only -E 'test(hiz_dust2)'
//! --no-capture`. Each eye's drawn instances on and off, and the same pixels.

use glam::Vec3;

use super::gpu_tests::{drawn, offscreen, shot, H, W};
use crate::scene::lighting::occlusion::bench_level::rooms;
use crate::scene::Camera;

#[test]
// lint: allow(skip) a measurement on the 6344-mesh level, run by hand, never a gate
#[ignore = "a measurement, run by hand"]
fn gpu_hiz_dust2_counts() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let scene = rooms(20);
    let eyes = [
        (
            "in a room, at a doorway",
            Vec3::new(55.0, 1.6, 52.0),
            0.0,
            0.0,
        ),
        (
            "down a row of doorways",
            Vec3::new(2.0, 1.6, 105.0),
            0.0,
            0.0,
        ),
        (
            "upstairs, at a doorway",
            Vec3::new(105.0, 5.6, 103.0),
            90.0,
            -5.0,
        ),
        (
            "above the level",
            Vec3::new(100.0, 30.0, -20.0),
            90.0,
            -35.0,
        ),
    ];
    for (name, at, yaw, pitch) in eyes {
        let cam = Camera::new(at, yaw, pitch);
        let mut view = offscreen(&r);
        r.quality.gpu_occlusion_culling = true;
        shot(&mut r, &mut view, &scene, &cam);
        let (on, c) = shot(&mut r, &mut view, &scene, &cam);
        let kept: u32 = drawn(&r, &view).iter().sum();
        r.quality.gpu_occlusion_culling = false;
        let mut fresh = offscreen(&r);
        let (off, _) = shot(&mut r, &mut fresh, &scene, &cam);
        let same = if on == off {
            "same pixels"
        } else {
            "PIXELS DIFFER"
        };
        println!(
            "{name}: {} meshes drawn by the CPU's culls, {} instances tested, {kept} drawn with Hi-Z, {same}",
            c.visible_entities, c.hiz_tested
        );
        assert!(on == off, "{name}: Hi-Z moved pixels");
    }
}
