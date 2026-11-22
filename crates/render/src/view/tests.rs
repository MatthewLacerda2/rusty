use glam::Vec3;

use super::RenderView;
use crate::OFFSCREEN_FORMAT;
use rusty_scene::create::{create_live_light, Primitive};
use rusty_scene::Camera;
use rusty_scene::{MeshComponent, Scene};

fn box_scene() -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    create_live_light(&mut scene, "Sun", Primitive::DirectionalLight);
    let id = scene.add_entity("Box".to_string());
    let (vertices, indices) = rusty_components::mesh::primitives::generate_box(1.0, 1.0, 1.0);
    scene
        .world
        .set_mesh(id, Some(MeshComponent::new("Box", vertices, indices)));
    scene
}

fn camera() -> Camera {
    Camera::new(Vec3::new(0.0, 0.0, 5.0), -90.0, 0.0)
}

#[test]
fn gpu_offscreen_view_owns_a_target_targetless_does_not() {
    let Some(renderer) = crate::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    let off = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, 32, 32);
    let none = RenderView::targetless(&renderer.device, OFFSCREEN_FORMAT, 32, 32);
    assert!(off.color_target_view().is_some());
    assert!(none.color_target_view().is_none());
}

/// One scenario on one `Renderer`, not three tests: each is a whole device,
/// held in RAM on lavapipe.
#[test]
fn gpu_two_views_two_scenes_never_clobber_each_other() {
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(64, 64) else {
        return;
    };

    let mut wide = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, 96, 48);
    let mut tall = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, 40, 80);
    let scene_a = box_scene();
    let scene_b = box_scene();
    let cam = camera();

    let out_a = wide.color_target_view().unwrap();
    renderer.render(&mut wide, &scene_a, &cam, &out_a, false);
    assert_eq!(renderer.material_group_count(), 1);
    assert!(
        !renderer.shadow_renderer.needs_static_bake(&scene_a),
        "scene A's statics are baked after its render"
    );
    assert!(
        renderer.shadow_renderer.needs_static_bake(&scene_b),
        "but scene B must bake its own rather than inherit A's"
    );

    let out_b = tall.color_target_view().unwrap();
    renderer.render(&mut tall, &scene_b, &cam, &out_b, false);

    assert_eq!((wide.size().width, wide.size().height), (96, 48));
    assert_eq!((tall.size().width, tall.size().height), (40, 80));

    assert_eq!(
        renderer.material_group_count(),
        1,
        "two scenes with one material look share one material bind group"
    );

    assert!(!renderer.shadow_renderer.needs_static_bake(&scene_b));
    assert!(renderer.shadow_renderer.needs_static_bake(&scene_a));

    let out_a = wide.color_target_view().unwrap();
    renderer.render(&mut wide, &scene_a, &cam, &out_a, false);
    let out_b = tall.color_target_view().unwrap();
    renderer.render(&mut tall, &scene_b, &cam, &out_b, false);
    assert_eq!((wide.size().width, wide.size().height), (96, 48));
    assert_eq!((tall.size().width, tall.size().height), (40, 80));
    assert_eq!(renderer.material_group_count(), 1, "no per-frame growth");
}

#[test]
fn gpu_resize_tracks_the_new_size() {
    let Some(renderer) = crate::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    let mut view = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, 32, 32);
    view.resize(&renderer.device, 50, 70);
    assert_eq!((view.size().width, view.size().height), (50, 70));
    view.resize(&renderer.device, 50, 70);
    assert_eq!((view.size().width, view.size().height), (50, 70));
}
