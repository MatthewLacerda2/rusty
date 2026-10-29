//! Automatic exposure through the whole frame (#1444): `Renderer::render` meters
//! the main camera's view, eases on the scene's game time in Play, holds while it is
//! paused and snaps outside Play. The limits pin each target, so the steps are exact
//! whatever the empty scene meters.

use glam::Vec3;

use crate::components::ExposureMode;
use crate::render::{RenderView, OFFSCREEN_FORMAT};
use crate::scene::authoring::default_post_processing;
use crate::scene::authoring::rendering::post_processing as ops;
use crate::scene::{Camera, Scene};

/// Pin the volume's target at `ev`.
fn target(scene: &mut Scene, id: u32, ev: f32) {
    let mut vc = scene.world.post_processing_mut(id).unwrap();
    ops::exposure::set_limits(&mut vc, ev, ev);
}

#[test]
fn gpu_a_rendered_view_eases_in_play_holds_when_paused_and_snaps_in_edit() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(16, 16) else {
        return;
    };
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, 16, 16);
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let id = scene.add_entity("Camera".to_string());
    scene.world.set_camera(id, Some(Default::default()));
    let mut vc = default_post_processing();
    ops::exposure::set_mode(&mut vc, ExposureMode::Automatic);
    scene.world.set_post_processing(id, Some(vc));
    let cam = Camera::new(Vec3::ZERO, -90.0, 0.0);
    let out = view.color_target_view().unwrap();
    let mut frame = |r: &mut crate::render::Renderer, scene: &Scene| {
        r.render(&mut view, scene, &cam, &out, false);
        r.wait_idle();
        view.settled_exposure(&r.device).expect("a metered frame")
    };

    target(&mut scene, id, -2.0);
    assert_eq!(frame(&mut r, &scene), -2.0, "edit mode snaps");
    target(&mut scene, id, 2.0);
    assert_eq!(frame(&mut r, &scene), 2.0, "edit mode snaps again");

    scene.set_playing(true);
    target(&mut scene, id, -2.0);
    scene.shader_time = 1.0;
    assert_eq!(frame(&mut r, &scene), -2.0, "Play is a cut");
    target(&mut scene, id, 2.0);
    scene.shader_time = 1.5;
    assert_eq!(frame(&mut r, &scene), -1.5, "eases at speed_up = 1 EV/s");
    assert_eq!(frame(&mut r, &scene), -1.5, "time scale 0 holds it");
    scene.shader_time = 2.0;
    assert_eq!(frame(&mut r, &scene), -1.0);

    scene.set_playing(false);
    scene.shader_time = 0.0;
    assert_eq!(frame(&mut r, &scene), 2.0, "Stop snaps");
}
