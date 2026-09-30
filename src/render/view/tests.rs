//! Tests for `RenderView` (#355): the per-view invariant that two views of two
//! scenes render in one frame without clobbering each other's size/targets — the
//! regression guard for the single-view assumption this type removes. GPU tests skip
//! gracefully when no adapter is present, the same contract as the screenshot path.

use glam::Vec3;

use super::RenderView;
use crate::render::OFFSCREEN_FORMAT;
use crate::scene::Camera;
use crate::scene::{DirtyFlag, MeshComponent, Scene};

fn box_scene() -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let id = scene.add_entity("Box".to_string());
    let (vertices, indices) = crate::components::mesh::primitives::generate_box(1.0, 1.0, 1.0);
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
fn gpu_offscreen_view_owns_a_target_targetless_does_not() {
    let Some(renderer) = crate::render::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    let off = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, 32, 32, 2);
    let none = RenderView::targetless(&renderer.device, OFFSCREEN_FORMAT, 32, 32, 2);
    assert!(off.color_target_view().is_some());
    assert!(none.color_target_view().is_none());
}

/// The #355 invariant, end to end: two views of two independent scenes, rendered
/// back to back through one shared renderer, clobber none of each other's state.
///
/// One scenario, one renderer, every axis asserted — deliberately not three tests.
/// Each `Renderer` is a full device + pipelines + shadow maps, and on a software
/// adapter (Windows CI's WARP renders into system RAM) enough concurrent ones
/// exhaust memory mid-suite. Sharing the scenario keeps the peak flat.
///
/// The three axes, and what each looked like when it was broken:
/// - **size** — the two views' resize guards defeated each other, reallocating both
///   targets twice per frame;
/// - **pooled entity resources** — both scenes call their entity 1, so keyed by
///   entity id alone each render evicted the other's slot and rebuilt it next frame.
///   Since #470 nothing is pooled per entity: draw data is packed per render, and the
///   one cached resource — the material bind group — is keyed by the maps it binds,
///   so both scenes' default-material boxes share one;
/// - **static shadows** — a bare `is_static_cached` bool let scene B skip its own
///   bake and sample scene A's shadow map (the phantom shadows on the preview mesh).
#[test]
fn gpu_two_views_two_scenes_never_clobber_each_other() {
    let Some(mut renderer) = crate::render::test_gpu::headless_or_skip(64, 64) else {
        return;
    };

    let mut wide = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, 96, 48, 2);
    let mut tall = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, 40, 80, 2);
    let scene_a = box_scene();
    let scene_b = box_scene();
    let cam = camera();

    // Render both views in the same frame, interleaved, as the editor viewport + the
    // Inspector preview do.
    let out_a = wide.color_target_view().unwrap();
    renderer.render(&mut wide, &scene_a, &cam, &out_a, false);
    assert_eq!(renderer.material_group_count(), 1);
    assert!(
        !renderer.shadow_renderer.needs_static_bake(scene_a.id()),
        "scene A's statics are baked after its render"
    );
    assert!(
        renderer.shadow_renderer.needs_static_bake(scene_b.id()),
        "but scene B must bake its own rather than inherit A's"
    );

    let out_b = tall.color_target_view().unwrap();
    renderer.render(&mut tall, &scene_b, &cam, &out_b, false);

    // Sizes: each view kept its own.
    assert_eq!((wide.size().width, wide.size().height), (96, 48));
    assert_eq!((tall.size().width, tall.size().height), (40, 80));

    // Cached resources: B's render reused A's material group rather than rebuilding.
    assert_eq!(
        renderer.material_group_count(),
        1,
        "two scenes with one material look share one material bind group"
    );

    // Static shadows: the map now holds B's statics, so A must re-bake.
    assert!(!renderer.shadow_renderer.needs_static_bake(scene_b.id()));
    assert!(renderer.shadow_renderer.needs_static_bake(scene_a.id()));

    // A second frame settles rather than thrashing: same sizes, same slot count.
    let out_a = wide.color_target_view().unwrap();
    renderer.render(&mut wide, &scene_a, &cam, &out_a, false);
    let out_b = tall.color_target_view().unwrap();
    renderer.render(&mut tall, &scene_b, &cam, &out_b, false);
    assert_eq!((wide.size().width, wide.size().height), (96, 48));
    assert_eq!((tall.size().width, tall.size().height), (40, 80));
    assert_eq!(renderer.material_group_count(), 1, "no per-frame growth");
}

/// `resize` reallocates the offscreen target to the new size and is a cheap no-op when
/// the size and quality divisor are unchanged.
#[test]
fn gpu_resize_tracks_the_new_size() {
    let Some(renderer) = crate::render::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    let mut view = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, 32, 32, 2);
    view.resize(&renderer.device, 50, 70, 2);
    assert_eq!((view.size().width, view.size().height), (50, 70));
    // Unchanged resize: still the same size, no panic.
    view.resize(&renderer.device, 50, 70, 2);
    assert_eq!((view.size().width, view.size().height), (50, 70));
}
