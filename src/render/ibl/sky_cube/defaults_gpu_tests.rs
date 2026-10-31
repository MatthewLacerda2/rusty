//! The default procedural sky against today's (#1620): the settings' defaults
//! (`cloud_shift(0.5)`, opacity 0.6, the derived colours) must draw the sky `main`
//! drew before they existed.

use glam::Vec3;

use super::gpu_tests::half;
use crate::render::{RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::{Camera, Scene};

/// The blurred mip the test reads (8×8 a face): low enough that an adapter's
/// rounding averages out, sharp enough that the clouds still move it.
const LEVEL: u32 = 4;

/// Each face's mean (+X, -X, +Y, -Y, +Z, -Z) at [`LEVEL`] of the default sky's cube:
/// the base term (rgb, grey) and the ambient weight (a). Taken on lavapipe with
/// `main`'s shaders at 01cfc8b (before the settings), the cube that had no settings.
const TODAY: [[f32; 2]; 6] = [
    [0.24927, 0.49021],
    [0.22787, 0.51199],
    [0.17551, 0.81300],
    [0.09415, 0.28105],
    [0.22067, 0.51921],
    [0.24931, 0.49049],
];

/// How far a face's mean may drift from [`TODAY`]: adapter rounding (the clouds'
/// hash is a `sin`, which drivers round differently), and under half of what a
/// 0.05 change to either cloud default moves (asserted below). Lavapipe matches
/// `TODAY` exactly.
const TOLERANCE: f32 = 1.5e-3;

/// Render one frame of `scene` and return the bound procedural cube's face means
/// at [`LEVEL`]: `[base, weight]` (the base's three channels must agree).
fn face_means(r: &mut Renderer, view: &mut RenderView, scene: &Scene) -> [[f32; 2]; 6] {
    let cam = Camera::new(Vec3::ZERO, -90.0, 0.0);
    let out = view.color_target_view().unwrap();
    r.render(view, scene, &cam, &out, false);
    let sky = &r.sky_reflection;
    let cube = &sky.procedural[sky.current].cube;
    let size = super::SKY_FACE >> LEVEL;
    std::array::from_fn(|face| {
        let level = (&cube.texture, LEVEL, face as u32);
        let bytes =
            crate::render::readback::read_mip_bytes(&r.device, &r.queue, level, (size, size), 8);
        let mut sum = [0.0f32; 4];
        for texel in bytes.chunks_exact(8) {
            for (c, s) in sum.iter_mut().enumerate() {
                *s += half(u16::from_le_bytes([texel[2 * c], texel[2 * c + 1]]));
            }
        }
        let [r, g, b, a] = sum.map(|s| s / (size * size) as f32);
        assert!((r - g).abs() < 1e-4 && (r - b).abs() < 1e-4, "grey base");
        [r, a]
    })
}

/// The largest per-face drift of `got` from [`TODAY`].
fn drift(got: &[[f32; 2]; 6]) -> f32 {
    got.iter()
        .flatten()
        .zip(TODAY.iter().flatten())
        .map(|(g, w)| (g - w).abs())
        .fold(0.0, f32::max)
}

#[test]
fn gpu_the_default_sky_is_todays() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(8, 8) else {
        return;
    };
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, 8, 8);
    let mut scene = Scene::new();
    let got = face_means(&mut r, &mut view, &scene);
    assert!(
        drift(&got) <= TOLERANCE,
        "default sky {got:?} vs today's {TODAY:?}"
    );

    // The tolerance still sees a small change to a default.
    scene.sky.cloud_coverage += 0.05;
    let moved = face_means(&mut r, &mut view, &scene);
    assert!(
        drift(&moved) > 4.0 * TOLERANCE,
        "coverage +0.05 moved {}",
        drift(&moved)
    );
    scene.sky.cloud_coverage -= 0.05;
    scene.sky.cloud_opacity += 0.05;
    let moved = face_means(&mut r, &mut view, &scene);
    assert!(
        drift(&moved) > 2.0 * TOLERANCE,
        "opacity +0.05 moved {}",
        drift(&moved)
    );
}
