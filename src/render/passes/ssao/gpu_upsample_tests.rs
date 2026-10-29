//! SSAO below full resolution (#1578): the blur runs at the occlusion's size and one
//! depth-aware upsample brings it to the view's, so a half- or quarter-resolution
//! AO darkens the same contact the full-resolution one does, and never the sky.
//! Skips when no adapter is present.

use glam::Vec3;

use crate::core::settings::quality::EffectQuality;
use crate::render::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::authoring::{create_entity, default_post_processing, Primitive};
use crate::scene::{Camera, Scene};

const W: u32 = 96;
const H: u32 = 64;

/// A crate on a floor under a volume with strong AO at `divisor` (`None`: AO off).
fn crate_on_floor(divisor: Option<u32>) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let floor = create_entity(&mut scene, "Floor", Some(Primitive::Box));
    let mut t = scene.world.transform_mut(floor).unwrap();
    (t.position, t.scale) = (Vec3::new(0.0, -0.5, 0.0), Vec3::new(20.0, 1.0, 20.0));
    drop(t);
    create_entity(&mut scene, "Crate", Some(Primitive::Box));
    let volume = create_entity(&mut scene, "Volume", None);
    let mut vc = default_post_processing();
    vc.ssao.active = divisor.is_some();
    (vc.ssao.intensity, vc.ssao.radius) = (4.0, 1.0);
    vc.ssao.quality = EffectQuality::Custom;
    (vc.ssao.samples, vc.ssao.resolution_divisor) = (16, divisor.unwrap_or(1));
    scene.world.set_post_processing(volume, Some(vc));
    let lens = crate::components::CameraComponent::default();
    scene.world.set_camera(volume, Some(lens));
    scene
}

/// The frame's RGBA8 pixels.
fn frame(r: &mut Renderer, scene: &Scene) -> Vec<u8> {
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H);
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(Vec3::new(0.0, 1.5, 3.0), -90.0, -25.0);
    r.render(&mut view, scene, &cam, &out, false);
    let target = view.color_target().unwrap();
    readback::read_texture_rgba8(&r.device, &r.queue, target, W, H)
}

/// How much darker `lit` is than `off`, summed over every channel.
fn darkening(off: &[u8], lit: &[u8]) -> i64 {
    off.iter()
        .zip(lit)
        .map(|(a, b)| i64::from(*a) - i64::from(*b))
        .sum()
}

#[test]
fn gpu_lower_resolution_ao_darkens_what_full_resolution_does() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let off = frame(&mut r, &crate_on_floor(None));
    let full = darkening(&off, &frame(&mut r, &crate_on_floor(Some(1))));
    assert!(full > 0, "full-resolution AO darkened nothing");
    for divisor in [2, 4] {
        let lit = frame(&mut r, &crate_on_floor(Some(divisor)));
        let low = darkening(&off, &lit);
        // The same contact, within what sampling it more coarsely may move.
        let (lo, hi) = (full / 2, full * 3 / 2);
        assert!(
            (lo..=hi).contains(&low),
            "divisor {divisor}: {low} vs {full}"
        );
        // The upsample never darkens a pixel no AO reaches: the sky stays the sky.
        let sky = |px: &[u8]| px[..4 * W as usize].to_vec();
        assert_eq!(sky(&off), sky(&lit), "divisor {divisor}: the top row moved");
    }
}
