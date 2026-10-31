//! The procedural sky's settings on a GPU (#1620): they reach the cube's bake, a
//! change bakes once, and coverage 0 is a cloud-free sky.

use glam::Vec3;

use super::gpu_tests::half;
use crate::render::{RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::{Camera, Scene, SkySettings};

/// Render one frame of `scene`; returns how many sky cubes it baked and the bound
/// procedural cube's mip-0 +Y face (the sky overhead, clouds and all), raw halves.
fn frame(r: &mut Renderer, view: &mut RenderView, scene: &Scene) -> (u32, Vec<u8>) {
    let cam = Camera::new(Vec3::ZERO, -90.0, 0.0);
    let out = view.color_target_view().unwrap();
    r.render(view, scene, &cam, &out, false);
    let sky = &r.sky_reflection;
    let cube = &sky.procedural[sky.current].cube;
    let size = super::SKY_FACE;
    let level = (&cube.texture, 0, 2);
    let face = crate::render::readback::read_mip_bytes(&r.device, &r.queue, level, (size, size), 8);
    (r.frame_counters.sky_reflection_bakes, face)
}

#[test]
fn gpu_sky_settings_rebake_once_and_coverage_zero_is_clear() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(8, 8) else {
        return;
    };
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, 8, 8);
    let mut scene = Scene::new();
    let (baked, default) = frame(&mut r, &mut view, &scene);
    assert_eq!(baked, 1, "the first procedural frame");

    scene.sky.cloud_coverage = 0.0;
    let (baked, clear) = frame(&mut r, &mut view, &scene);
    assert_eq!(baked, 1, "a settings change bakes once");
    assert_ne!(clear, default, "the default sky has clouds overhead");
    assert_eq!(frame(&mut r, &mut view, &scene).0, 0, "an unchanged sky");

    scene.sky = SkySettings::default();
    assert_eq!(
        frame(&mut r, &mut view, &scene),
        (0, default.clone()),
        "the sky bound before is still held"
    );

    // No clouds either way: coverage 0 carves none, opacity 0 hides them.
    scene.sky.cloud_opacity = 0.0;
    let (_, hidden) = frame(&mut r, &mut view, &scene);
    assert_eq!(hidden, clear, "coverage 0 is the cloud-free sky");

    // Every colour set to what the defaults derive draws the default sky.
    let [zenith, horizon, ground] = SkySettings::default().resolved(scene.ambient_color);
    scene.sky = SkySettings {
        zenith: Some(zenith),
        horizon: Some(horizon),
        ground: Some(ground),
        ..SkySettings::default()
    };
    let (_, explicit) = frame(&mut r, &mut view, &scene);
    // The set colours' cube holds no ambient term, so compare the drawn sky: the
    // terms' rgb plus the ambient times their alpha.
    let ambient = scene.ambient_color;
    let drawn = |face: &[u8]| -> Vec<Vec3> {
        face.chunks_exact(8)
            .map(|t| {
                let h = |i: usize| half(u16::from_le_bytes([t[2 * i], t[2 * i + 1]]));
                Vec3::new(h(0), h(1), h(2)) + ambient * h(3)
            })
            .collect()
    };
    for (w, g) in drawn(&default).iter().zip(&drawn(&explicit)) {
        assert!(w.abs_diff_eq(*g, 2e-3), "default {w} vs explicit {g}");
    }
}
