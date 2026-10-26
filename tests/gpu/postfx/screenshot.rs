//! Post-FX visual smoke test (issue #14). When a GPU / software adapter (e.g.
//! lavapipe) is present, render the same scene twice with different exposure and
//! assert the average pixel brightness actually changes — i.e. the color-
//! correction knob reaches the image. When no adapter is available, `capture`
//! returns `Ok(false)` and the test passes without asserting (GPU verification
//! simply wasn't possible in this environment).

use crate::gpu::mean_brightness;
use crate::temp::file as tmp;
use glam::Vec3;
use rusty::dev::screenshot::capture;
use rusty::scene::Camera;
use rusty::scene::{MeshComponent, Scene, VisualCorrectionComponent};

fn vc(exposure: f32) -> VisualCorrectionComponent {
    VisualCorrectionComponent {
        bloom_threshold: 0.6,
        exposure,
        ssr_active: false,
        ssr_temporal_upsampling: false,
        ..rusty::scene::authoring::default_visual_correction()
    }
}

fn scene_with_exposure(exposure: f32) -> Scene {
    let mut scene = Scene::new();
    scene.ambient_intensity = 1.0;
    let id = scene.add_entity("Box".to_string());
    let (vertices, indices) = rusty::components::mesh::primitives::generate_box(2.0, 2.0, 2.0);
    scene
        .world
        .set_mesh(id, Some(MeshComponent::new("Box", vertices, indices)));
    scene.world.set_visual_correction(id, Some(vc(exposure)));
    // A volume rides on its camera (#1434): that camera is the view's.
    scene
        .world
        .set_camera(id, Some(rusty::components::CameraComponent::default()));
    scene
}

#[test]
fn exposure_visibly_changes_the_rendered_image() {
    let cam = Camera::new(Vec3::new(0.0, 0.0, 5.0), 180.0, 0.0);

    let dark = tmp("rusty_postfx_dark.png");
    let bright = tmp("rusty_postfx_bright.png");

    let low = scene_with_exposure(-2.0);
    let high = scene_with_exposure(2.0);

    let captured_low = capture(&low, &cam, &dark, 96, 96).expect("capture must not error");
    let captured_high = capture(&high, &cam, &bright, 96, 96).expect("capture must not error");

    if !captured_low || !captured_high {
        eprintln!("[postfx] no GPU/software adapter — skipping visual assertion");
        return;
    }

    let b_dark = mean_brightness(&dark);
    let b_bright = mean_brightness(&bright);
    eprintln!("[postfx] mean brightness: exposure -2 = {b_dark:.2}, +2 = {b_bright:.2}");
    assert!(
        b_bright > b_dark + 1.0,
        "higher exposure should brighten the image (dark={b_dark:.2}, bright={b_bright:.2})"
    );
}
