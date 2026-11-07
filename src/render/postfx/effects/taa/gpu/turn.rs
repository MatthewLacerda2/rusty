//! #1846 on lavapipe: a camera turning at 90°/s (1.5° a frame at 60 fps) across a
//! wall of noise keeps its detail. URP's fixed frame influence resamples 90 % of
//! every pixel from the reprojected history each frame, and the image softens
//! while turning; a moving pixel's higher weight keeps it near the still frame.

use super::fixture::{eye, frame, renderer, settle, taa_scene, view};
use super::wall::{luma, noisy_wall};
use crate::render::motion::gpu_fixture::{H, W};
use crate::render::{RenderView, Renderer};
use crate::scene::Scene;

/// Degrees a frame: 90°/s at 60 fps.
const STEP: f32 = 1.5;
/// Frames turned, ending at yaw 0, and frames the still view settles.
const FRAMES: u32 = 16;
/// The turning frame's sharpness over the still one's must reach this. Measured
/// on lavapipe: 0.93 with the moving weight, 0.76 with URP's fixed one (the
/// moving weight set to URP's).
const KEPT: f32 = 0.85;

/// The mean luma step between neighbouring pixels across the frame's middle,
/// away from its edges (where a turn brings in fresh pixels).
fn sharpness(rgba: &[u8]) -> f32 {
    let l = luma(rgba);
    let at = |x: u32, y: u32| l[(y * W + x) as usize];
    let (mut sum, mut n) = (0.0, 0.0);
    for y in H / 4..H * 3 / 4 {
        for x in W / 4..W * 3 / 4 {
            sum += (at(x + 1, y) - at(x, y)).abs() + (at(x, y + 1) - at(x, y)).abs();
            n += 1.0;
        }
    }
    sum / n
}

fn wall_scene() -> Scene {
    let (mut scene, _) = taa_scene(true, true);
    noisy_wall(&mut scene, -7.0);
    scene
}

/// The last frame of `FRAMES` turning toward yaw 0 in `v`.
fn turned(r: &mut Renderer, v: &mut RenderView, scene: &Scene) -> Vec<u8> {
    let yaw = |f: u32| (f as f32 + 1.0 - FRAMES as f32) * STEP;
    (0..FRAMES - 1).for_each(|f| drop(frame(r, v, scene, &eye(yaw(f)))));
    frame(r, v, scene, &eye(0.0))
}

#[test]
fn gpu_a_turning_camera_keeps_the_still_frames_sharpness() {
    let Some(mut r) = renderer() else { return };
    let scene = wall_scene();
    let mut still_view = view(&r);
    let still = settle(&mut r, &mut still_view, &scene, &eye(0.0), FRAMES);
    let mut turning_view = view(&r);
    let turning = turned(&mut r, &mut turning_view, &scene);
    let (s, t) = (sharpness(&still), sharpness(&turning));
    eprintln!(
        "[taa] sharpness: still {s:.2}, turning {t:.2} ({:.2})",
        t / s
    );
    assert!(
        t >= KEPT * s,
        "turning keeps {:.2} of the still frame's sharpness",
        t / s
    );
}
