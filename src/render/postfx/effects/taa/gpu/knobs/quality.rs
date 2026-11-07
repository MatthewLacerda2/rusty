//! #1847 on lavapipe: every TAA quality level and every float knob at its extremes
//! resolves a sane frame (lit, not black) under a turning camera, and each level
//! whose URP steps differ from `High`'s draws a different frame (a still frame
//! would hide most of them: with no motion the history reads at texel centres,
//! where bilinear and bicubic agree).

use glam::Vec3;

use super::super::fixture::{eye, frame, renderer, taa_scene, tilted_cube, view};
use super::super::still::differing;
use super::super::wall::{luma, noisy_wall};
use crate::components::{CameraComponent, TaaQuality};
use crate::render::Renderer;
use crate::scene::authoring::rendering::camera::TAA_KNOBS;
use crate::scene::Scene;

/// Frames turned, 1.5° each, ending at yaw 0.
const FRAMES: u32 = 12;

/// The noise wall and a tilted cube, the camera running TAA tuned by `tune`.
fn tuned(tune: impl FnOnce(&mut CameraComponent)) -> Scene {
    let (mut scene, cam) = taa_scene(true, true);
    noisy_wall(&mut scene, -7.0);
    tilted_cube(&mut scene, Vec3::new(0.0, 0.0, -4.0));
    tune(&mut *scene.world.camera_mut(cam).unwrap());
    scene
}

/// The last frame of a turn toward yaw 0, in a fresh view.
fn turned(r: &mut Renderer, scene: &Scene) -> Vec<u8> {
    let mut v = view(r);
    let yaw = |f: u32| (f as f32 + 1.0 - FRAMES as f32) * 1.5;
    (0..FRAMES - 1).for_each(|f| drop(frame(r, &mut v, scene, &eye(yaw(f)))));
    frame(r, &mut v, scene, &eye(0.0))
}

/// The frame's mean luma.
fn mean_luma(rgba: &[u8]) -> f32 {
    let l = luma(rgba);
    l.iter().sum::<f32>() / l.len() as f32
}

/// That `got` is as lit as `reference`, within a quarter: no black or blown frame.
fn assert_sane(got: &[u8], reference: &[u8], what: &str) {
    let (g, r) = (mean_luma(got), mean_luma(reference));
    assert!(
        g > r * 0.75 && g < r * 1.25,
        "{what}: mean luma {g} against High's {r}"
    );
}

#[test]
fn gpu_each_quality_level_resolves_and_differs_from_high() {
    let Some(mut r) = renderer() else { return };
    let high = turned(&mut r, &tuned(|_| {}));
    assert!(mean_luma(&high) > 10.0, "the reference frame is lit");
    for quality in TaaQuality::ALL {
        let got = turned(&mut r, &tuned(|c| c.taa_quality = quality));
        assert_sane(&got, &high, quality.name());
        let changed = differing(&got, &high);
        if quality == TaaQuality::High {
            assert_eq!(changed, 0, "High is the default");
        } else {
            assert!(changed > 0, "{} draws as High does", quality.name());
        }
    }
}

#[test]
fn gpu_each_float_knob_resolves_at_its_extremes() {
    let Some(mut r) = renderer() else { return };
    let default = turned(&mut r, &tuned(|_| {}));
    for knob in TAA_KNOBS {
        let at_default = (knob.get)(&CameraComponent::default());
        for value in [knob.range.0, knob.range.1] {
            let got = turned(&mut r, &tuned(|c| (knob.set)(c, value)));
            let what = format!("{} at {value}", knob.field);
            assert_sane(&got, &default, &what);
            // The mip bias moves only a mipped map's detail, which the wall's
            // 32² texture under this view may not show; every other knob away from
            // its default changes the frame.
            if value != at_default && knob.field != "mip_bias" {
                assert!(differing(&got, &default) > 0, "{what} changed nothing");
            }
        }
    }
}
