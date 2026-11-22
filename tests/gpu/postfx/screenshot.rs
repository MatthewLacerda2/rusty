//! The same scene at two exposures must differ in mean brightness: the colour-correction
//! knob reaches the image.

use crate::gpu::mean_brightness;
use crate::temp::file as tmp;
use glam::Vec3;
use rusty::dev::screenshot::capture;
use rusty::scene::Camera;
use rusty::scene::{MeshComponent, PostProcessingComponent, Scene};

fn vc(exposure: f32) -> PostProcessingComponent {
    PostProcessingComponent {
        bloom: rusty::components::Bloom {
            threshold: 0.6,
            ..Default::default()
        },
        exposure: rusty::components::Exposure {
            fixed_ev: exposure,
            ..Default::default()
        },
        ssr: rusty::components::Ssr {
            active: false,
            ..Default::default()
        },
        ..rusty::edit::authoring::default_post_processing()
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
    scene.world.set_post_processing(id, Some(vc(exposure)));
    // A volume rides on its camera: that camera is the view's.
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
