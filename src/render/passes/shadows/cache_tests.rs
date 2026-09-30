//! The per-cascade static-bake cache (#355, #435), on the GPU: a still camera re-draws
//! no static caster, a nudge inside the snap grid re-draws none either, and a real
//! move re-bakes the cascades it shifted. Skips when no adapter is present.

use glam::Vec3;

use crate::render::{RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::{Camera, Scene};

/// One static box under the default sun, and nothing that moves.
fn static_yard() -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    create_entity(&mut scene, "Sun", Some(Primitive::DirectionalLight));
    let id = create_entity(&mut scene, "Crate", Some(Primitive::Box));
    scene.world.set_static(id, true);
    scene
}

/// Shadow draws submitted rendering `scene` from `position`.
fn shadow_draws(r: &mut Renderer, view: &mut RenderView, scene: &Scene, position: Vec3) -> u32 {
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(position, -90.0, -10.0);
    r.render(view, scene, &cam, &out, false);
    r.frame_counters.shadow_draws
}

#[test]
fn gpu_static_casters_are_baked_once_per_cascade_position() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(48, 32) else {
        return;
    };
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, 48, 32, 2);
    let scene = static_yard();
    let home = Vec3::new(0.0, 2.0, 6.0);

    assert!(
        shadow_draws(&mut r, &mut view, &scene, home) > 0,
        "first frame bakes"
    );
    assert_eq!(
        shadow_draws(&mut r, &mut view, &scene, home),
        0,
        "still: cached"
    );
    let nudged = home + Vec3::new(0.01, 0.0, 0.01);
    assert_eq!(
        shadow_draws(&mut r, &mut view, &scene, nudged),
        0,
        "inside the snap grid"
    );
    let moved = home + Vec3::new(0.0, 0.0, 20.0);
    assert!(
        shadow_draws(&mut r, &mut view, &scene, moved) > 0,
        "a moved cascade re-bakes"
    );
}
