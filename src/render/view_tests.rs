//! Tests for `RenderView` (#355): the per-view invariant that two views of two
//! scenes render in one frame without clobbering each other's size/targets — the
//! regression guard for the single-view assumption this type removes. GPU tests skip
//! gracefully when no adapter is present, the same contract as the screenshot path.

use glam::Vec3;

use super::RenderView;
use crate::render::{Camera, Renderer, OFFSCREEN_FORMAT};
use crate::scene::{DirtyFlag, MeshComponent, Scene};

fn box_scene() -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let id = scene.add_entity("Box".to_string());
    let (vertices, indices) = crate::render::gpu::mesh::generate_box(1.0, 1.0, 1.0);
    scene.world.set_mesh(
        id,
        Some(MeshComponent {
            primitive_type: "Box".to_string(),
            asset_ref: None,
            vertices,
            indices,
            bind_palette: Vec::new(),
            skin: None,
            clips: Vec::new(),
            pose_palette: Vec::new(),
            is_dirty: DirtyFlag::new(true),
        }),
    );
    scene
}

fn camera() -> Camera {
    Camera::new(Vec3::new(0.0, 0.0, 5.0), -90.0, 0.0)
}

/// An offscreen view exposes an owned colour target; a targetless one does not.
#[test]
fn offscreen_view_owns_a_target_targetless_does_not() {
    let Some(renderer) = pollster::block_on(Renderer::new_headless(32, 32)) else {
        return;
    };
    let off = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, 32, 32, 2);
    let none = RenderView::targetless(&renderer.device, OFFSCREEN_FORMAT, 32, 32, 2);
    assert!(off.color_target_view().is_some());
    assert!(none.color_target_view().is_none());
}

/// The core #355 invariant: two views of two independent scenes, rendered back to
/// back in one frame through one shared renderer, each keep their own size — neither
/// resizes to the other's dimensions (the double-realloc "resize fight" is gone).
#[test]
fn two_views_two_scenes_keep_independent_sizes() {
    let Some(mut renderer) = pollster::block_on(Renderer::new_headless(64, 64)) else {
        return;
    };

    let mut wide = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, 96, 48, 2);
    let mut tall = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, 40, 80, 2);
    let scene_a = box_scene();
    let scene_b = box_scene();
    let cam = camera();

    // Render both views in the same frame, interleaved, as the editor viewport + the
    // Inspector preview do. Before #355 this permanently reallocated both targets.
    let out_a = wide.color_target_view().unwrap();
    renderer.render(&mut wide, &scene_a, &cam, &out_a, false, &[]);
    let out_b = tall.color_target_view().unwrap();
    renderer.render(&mut tall, &scene_b, &cam, &out_b, false, &[]);

    assert_eq!(wide.size().width, 96);
    assert_eq!(wide.size().height, 48);
    assert_eq!(tall.size().width, 40);
    assert_eq!(tall.size().height, 80);

    // A second frame in the same order must still leave each view at its own size.
    let out_a = wide.color_target_view().unwrap();
    renderer.render(&mut wide, &scene_a, &cam, &out_a, false, &[]);
    let out_b = tall.color_target_view().unwrap();
    renderer.render(&mut tall, &scene_b, &cam, &out_b, false, &[]);
    assert_eq!((wide.size().width, wide.size().height), (96, 48));
    assert_eq!((tall.size().width, tall.size().height), (40, 80));
}

/// The per-scene half of the #355 invariant: rendering a second scene must not evict
/// the first scene's pooled per-entity GPU resources.
///
/// Both scenes here hold one entity, and both call it entity 1 — every fresh `World`
/// counts from 1. Keyed by entity id alone, the second render's prune dropped the
/// first scene's slot and re-inserted its own under the same key, so the two scenes
/// rebuilt each other's pool every frame — re-introducing exactly the per-frame churn
/// #210 removed. Keyed by (scene, entity), both survive.
#[test]
fn two_scenes_keep_their_own_pool_slots() {
    let Some(mut renderer) = pollster::block_on(Renderer::new_headless(64, 64)) else {
        return;
    };
    let mut view_a = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, 64, 64, 2);
    let mut view_b = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, 64, 64, 2);
    let scene_a = box_scene();
    let scene_b = box_scene();
    let cam = camera();

    let out = view_a.color_target_view().unwrap();
    renderer.render(&mut view_a, &scene_a, &cam, &out, false, &[]);
    assert_eq!(renderer.scene_slot_count(scene_a.id()), 1);

    // Rendering the second scene must leave the first scene's slot untouched.
    let out = view_b.color_target_view().unwrap();
    renderer.render(&mut view_b, &scene_b, &cam, &out, false, &[]);
    assert_eq!(
        renderer.scene_slot_count(scene_a.id()),
        1,
        "scene A's pooled resources survived scene B's render"
    );
    assert_eq!(renderer.scene_slot_count(scene_b.id()), 1);
    assert_eq!(
        renderer.entity_slot_count(),
        2,
        "two scenes' entity 1s are two slots, not one collided one"
    );

    // And a second frame settles rather than thrashing: still exactly two slots.
    let out = view_a.color_target_view().unwrap();
    renderer.render(&mut view_a, &scene_a, &cam, &out, false, &[]);
    let out = view_b.color_target_view().unwrap();
    renderer.render(&mut view_b, &scene_b, &cam, &out, false, &[]);
    assert_eq!(renderer.entity_slot_count(), 2);
}

/// `resize` reallocates the offscreen target to the new size and is a cheap no-op when
/// the size and quality divisor are unchanged.
#[test]
fn resize_tracks_the_new_size() {
    let Some(renderer) = pollster::block_on(Renderer::new_headless(32, 32)) else {
        return;
    };
    let mut view = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, 32, 32, 2);
    view.resize(&renderer.device, 50, 70, 2);
    assert_eq!((view.size().width, view.size().height), (50, 70));
    // Unchanged resize: still the same size, no panic.
    view.resize(&renderer.device, 50, 70, 2);
    assert_eq!((view.size().width, view.size().height), (50, 70));
}
