//! An inactive parent hides its subtree from the renderer (#1363): the issue's
//! holstered weapon, a pivot with two mesh children, draws nothing while the pivot is
//! inactive, though each child's own flag stays on. Skips when no adapter is present.

use glam::Vec3;

use crate::render::{RenderCounters, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::{Camera, Scene};

const W: u32 = 64;
const H: u32 = 48;

/// A pivot holding two meshes in front of the camera; returns the scene and pivot.
fn holstered() -> (Scene, u32) {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let pivot = create_entity(&mut scene, "Pivot", None);
    for (n, x) in [-0.8f32, 0.8].into_iter().enumerate() {
        let part = create_entity(&mut scene, &format!("Part_{n}"), Some(Primitive::Box));
        scene.world.transform_mut(part).unwrap().position = Vec3::new(x, 0.0, 0.0);
        scene.set_parent(part, Some(pivot)).unwrap();
    }
    (scene, pivot)
}

fn counters(renderer: &mut Renderer, scene: &Scene) -> RenderCounters {
    let mut view = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, W, H);
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(Vec3::new(0.0, 0.0, 5.0), -90.0, 0.0);
    renderer.render(&mut view, scene, &cam, &out, false);
    renderer.frame_counters
}

#[test]
fn gpu_children_of_an_inactive_pivot_draw_nothing() {
    let Some(mut renderer) = crate::render::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let (mut scene, pivot) = holstered();
    let drawn = counters(&mut renderer, &scene);
    assert_eq!(drawn.visible_entities, 2, "both parts on screen: {drawn:?}");
    scene.world.set_active(pivot, false);
    let hidden = counters(&mut renderer, &scene);
    assert_eq!(
        hidden.visible_entities, 0,
        "the pivot hides both: {hidden:?}"
    );
    assert_eq!(hidden.triangles, 0);
    let parts = scene.world.children(pivot);
    assert!(parts.iter().all(|&p| scene.world.is_active_self(p)));
}

#[test]
fn gpu_a_pivot_hidden_from_the_first_frame_still_uploads_its_meshes() {
    let Some(mut renderer) = crate::render::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let (mut scene, pivot) = holstered();
    scene.world.set_active(pivot, false);
    counters(&mut renderer, &scene);
    // Only drawing follows activity: showing the weapon later must not stall on
    // uploads, so the box is resident before it is ever drawn.
    assert!(
        !renderer.gpu_meshes.is_empty(),
        "the hidden box is uploaded"
    );
}
