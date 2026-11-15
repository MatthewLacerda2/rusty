//! The radius field reads light as the frame is shown, not in absolute units
//! (#2249): [`room`] lit by a lamp at the bulb, with the lamp and the bulb at
//! 0.01, 1 and 100 times their light, under automatic exposure. The radius field
//! weighs each hit by its brightness at the exposure the frame is shown at, so
//! the SSR-resolved image scales with the light and nothing else: a dark scene
//! exposed up keeps the field a lit one has. Read in absolute light, every hit of
//! the dim room sat at the floor and the field fell back to a plain mean.

use glam::Vec3;

use super::jitter::{room, H, TILE, W};

mod cross;
mod flash;
mod reference;
use crate::readback::read_texture_bytes;
use crate::{RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_components::{ExposureMode, LightComponent};
use rusty_core::color::f16_to_f32;
use rusty_scene::create::default_light;
use rusty_scene::{Camera, Scene};

/// [`room`] under a black sky, lit only by the bulb's glow and a lamp under it,
/// both `scale` times their light, exposed automatically within ±12 EV.
fn lit(scale: f32) -> Scene {
    let mut s = room();
    let cam = s.find_entity_by_name("Camera").unwrap();
    let mut volume = s.world.post_processing(cam).unwrap().clone();
    volume.exposure.mode = ExposureMode::Automatic;
    (
        volume.exposure.automatic.min_ev,
        volume.exposure.automatic.max_ev,
    ) = (-12.0, 12.0);
    s.world.set_post_processing(cam, Some(volume));
    crate::test_gpu::black_sky(&mut s);
    let bulb = s.materials.get_mut("Bulb").unwrap();
    bulb.emissive = bulb.emissive.map(|c| c * scale);
    let lamp = s.add_entity("Lamp".into());
    s.world.transform_mut(lamp).unwrap().position = Vec3::new(0.0, 2.4, -3.0);
    let light = LightComponent {
        intensity: 3.0 * scale,
        range: 20.0,
        cast_shadows: false,
        ..default_light()
    };
    s.world.set_light(lamp, Some(light));
    s
}

/// The SSR-resolved image of [`lit`] at `scale`: each tile's mean luma, divided
/// by `scale`, on the fourth frame (the first is metered and snaps the exposure,
/// which the SSR pass reads from the second on).
fn resolved(r: &mut Renderer, scale: f32) -> Vec<f32> {
    let mut v = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H);
    let eye = Camera::new(Vec3::new(0.0, 1.6, 0.0), -90.0, 30.0);
    let scene = lit(scale);
    for _ in 0..4 {
        let target = v.color_target_view().unwrap();
        r.render(&mut v, &scene, &eye, &target, false);
    }
    let bytes = read_texture_bytes(&r.device, &r.queue, &v.post_fx.scene_hdr.texture, (W, H), 8);
    let half = |i: usize| f16_to_f32(u16::from_le_bytes([bytes[i], bytes[i + 1]]));
    let luma = |x: u32, y: u32| {
        let i = ((y * W + x) * 8) as usize;
        0.25 * half(i) + 0.5 * half(i + 2) + 0.25 * half(i + 4)
    };
    let tile = |tx: u32, ty: u32| {
        let px = (0..TILE * TILE).map(|k| luma(tx * TILE + k % TILE, ty * TILE + k / TILE));
        px.sum::<f32>() / (TILE * TILE) as f32 / scale
    };
    let cols = W / TILE;
    (0..cols * (H / TILE))
        .map(|t| tile(t % cols, t / cols))
        .collect()
}

/// The most a tile of the resolved image, divided by its light, may differ from
/// the 1x scene's, in 255ths of perceptual luma at the 1x scene's light. Measured
/// on an RTX 2060, lavapipe and Metal: 0.01 at 0.01x, 0.00 to 0.01 at 100x; 4.5 at
/// 0.01x on the RTX 2060 with the hits' brightness read in absolute light.
const SAME: f32 = 0.5;

#[test]
fn gpu_the_radius_field_is_the_same_in_a_dim_scene_as_in_a_bright_one() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let one = resolved(&mut r, 1.0);
    let perceptual = |l: f32| 255.0 * l / (1.0 + l);
    for scale in [0.01, 100.0] {
        let other = resolved(&mut r, scale);
        let apart = one
            .iter()
            .zip(&other)
            .map(|(a, b)| (perceptual(*a) - perceptual(*b)).abs())
            .fold(0.0, f32::max);
        eprintln!(
            "[ssr] the room at {scale}x its light resolves within {apart:.2} of 1x, scaled back"
        );
        assert!(
            apart <= SAME,
            "the radius field changes with the scene's absolute light: {apart:.2} at {scale}x"
        );
    }
}
