//! The frame's GPU-call counts (#2341, `gpu::ops`) on a still primitive scene: a
//! frame that changes nothing makes the same calls as the one before, pinned, so a
//! change that adds a pass, a submit or a bind group per frame shows.

use glam::{Quat, Vec3};

use super::OpCounts;
use crate::{RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_scene::create::{create_entity, Primitive};
use rusty_scene::{Camera, Scene};

/// What each still frame after the first makes, at the default settings (the GPU
/// occlusion cull off, as every seeded preset ships it, #2388): the sun's four
/// dynamic cascade passes, the scene pass and two post-FX passes; no compute pass;
/// and three bind groups the post chain rebuilds every frame (its IO group for each
/// of its two passes, its effect textures for one), today's number, pinned so one
/// more shows and one fewer is a fix to re-pin.
/// Measured: lavapipe 7/0/3/3/19.
const STILL: OpCounts = OpCounts {
    render_passes: 7,
    compute_passes: 0,
    submits: 3,
    bind_groups_created: 3,
    buffer_writes: 19,
};

/// [`STILL`] with the GPU occlusion cull on: Hi-Z phase one adds a render pass, the
/// pyramid and the cull their compute passes, with a submit and three writes.
/// Measured: RTX 2060 8/4/4/3/22, lavapipe 8/4/4/3/22, Metal 8/4/4/3/22.
const STILL_CULLING: OpCounts = OpCounts {
    render_passes: 8,
    compute_passes: 4,
    submits: 4,
    bind_groups_created: 3,
    buffer_writes: 22,
};

const W: u32 = 64;
const H: u32 = 48;

/// A sun, a floor and three props: the forward pass, the sun's cascades, the
/// default post chain and, when it is on, the GPU occlusion cull all run.
fn still_scene() -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let sun = create_entity(&mut scene, "Sun", Some(Primitive::DirectionalLight));
    scene.world.transform_mut(sun).unwrap().rotation = Quat::from_rotation_x(-1.0);
    let floor = create_entity(&mut scene, "Floor", Some(Primitive::Plane));
    scene.world.transform_mut(floor).unwrap().scale = Vec3::splat(4.0);
    let kinds = [Primitive::Box, Primitive::Sphere, Primitive::Cylinder];
    for (n, kind) in kinds.into_iter().enumerate() {
        let id = create_entity(&mut scene, &format!("Prop_{n}"), Some(kind));
        scene.world.transform_mut(id).unwrap().position = Vec3::new(n as f32 - 1.0, 0.5, 0.0);
    }
    scene
}

/// `n` frames' op counts, and the last frame's `hiz_tested`.
fn frames(renderer: &mut Renderer, scene: &Scene, n: usize) -> (Vec<OpCounts>, u32) {
    let mut view = RenderView::offscreen(&renderer.device, OFFSCREEN_FORMAT, W, H);
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(Vec3::new(0.0, 3.0, 5.0), -90.0, -25.0);
    let ops = (0..n)
        .map(|_| {
            renderer.render(&mut view, scene, &cam, &out, false);
            renderer.frame_counters.ops
        })
        .collect();
    (ops, renderer.frame_counters.hiz_tested)
}

#[test]
fn gpu_a_still_frame_makes_the_calls_the_one_before_made() {
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let scene = still_scene();
    let (counts, tested) = frames(&mut renderer, &scene, 4);
    assert_eq!(counts[1..], [STILL; 3], "every still frame: {counts:#?}");
    assert_eq!(tested, 0, "a seeded preset runs no Hi-Z stage");
    // The first frame also builds what the later ones reuse.
    assert!(counts[0].bind_groups_created > STILL.bind_groups_created);
}

#[test]
fn gpu_a_still_frame_with_the_gpu_cull_on_makes_its_stage_s_calls() {
    let Some(mut renderer) = crate::test_gpu::culling::culling_or_skip(W, H) else {
        return;
    };
    let scene = still_scene();
    let (counts, tested) = frames(&mut renderer, &scene, 4);
    assert_eq!(
        counts[1..],
        [STILL_CULLING; 3],
        "every still frame: {counts:#?}"
    );
    assert!(tested > 0, "the floor and the props are tested");
}
