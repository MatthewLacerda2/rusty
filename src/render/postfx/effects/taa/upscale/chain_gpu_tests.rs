//! Where TAAU sits in the chain on lavapipe (#1702, part 3): the chain after it
//! runs at the output's size and FXAA skips on an upscaling frame, and a view that
//! stops upscaling runs the chain at the scene's size again.

use glam::Vec3;

use super::gpu_tests::{scaled, upscales};
use crate::render::motion::gpu_fixture::{H, W};
use crate::render::postfx::effects::taa::gpu_fixture::{
    eye, frame, renderer, settle, taa_scene, tilted_cube,
};

#[test]
fn gpu_an_upscaling_frame_runs_the_chain_at_the_output_size_and_skips_fxaa() {
    let Some(mut r) = renderer() else { return };
    let cam = eye(0.0);
    let at = Vec3::new(0.0, 0.0, -4.0);
    let (mut fxaa, _) = taa_scene(true, true);
    tilted_cube(&mut fxaa, at);
    let (mut plain, _) = taa_scene(true, false);
    tilted_cube(&mut plain, at);
    let mut a = scaled(&r, 0.5);
    let with = settle(&mut r, &mut a, &fxaa, &cam, 6);
    let mut b = scaled(&r, 0.5);
    let without = settle(&mut r, &mut b, &plain, &cam, 6);
    assert!(upscales(&a) && a.post_fx.taa_upscaled() && a.post_fx.taa_ran());
    assert_eq!(
        a.post_fx.chain_size,
        (W, H),
        "the chain after TAA at the output's size"
    );
    assert!(
        with == without,
        "FXAA on or off, an upscaling frame is TAA's alone"
    );

    // TAA off: the bilinear path, the chain back at the scene's size.
    let (mut off, _) = taa_scene(false, true);
    tilted_cube(&mut off, at);
    drop(frame(&mut r, &mut a, &off, &cam));
    assert!(!a.post_fx.taa_upscaled());
    let scene = a.scene_size();
    assert_eq!(a.post_fx.chain_size, (scene.width, scene.height));
}
