//! GPU tests for the kept batch groups (#2354): a still HUD makes none, and one is
//! made again when what it binds is replaced: the batch uniform outgrowing its
//! buffer, or a Mask's texture re-made at a new size.

use rusty_components::RectMaskComponent;
use rusty_scene::Scene;

use super::gpu_fixture::{element, minimap, Frames, SIZE};
use crate::Renderer;

/// The bind groups `scene`'s next frame makes.
fn made(renderer: &mut Renderer, frames: &mut Frames, scene: &Scene) -> u32 {
    frames.frame(renderer, scene);
    renderer.frame_counters.ops.bind_groups_created
}

#[test]
fn gpu_a_still_hud_makes_no_batch_group_and_a_replaced_resource_remakes_one() {
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(SIZE, SIZE) else {
        return;
    };
    // What a still frame makes with no UI at all: the post chain's groups.
    let mut empty = Frames::new(&renderer);
    made(&mut renderer, &mut empty, &Scene::new());
    let base = made(&mut renderer, &mut empty, &Scene::new());

    let (mut scene, panel, mask, _) = minimap();
    let mut frames = Frames::new(&renderer);
    made(&mut renderer, &mut frames, &scene);
    assert_eq!(
        made(&mut renderer, &mut frames, &scene),
        base,
        "still: none"
    );

    // Twenty more clipped batches outgrow the uniform's first buffer (16 slots).
    for n in 0..20 {
        let clip = element(&mut scene, panel, [n as f32, 0.0, 2.0, 2.0]);
        scene
            .world
            .set_rect_mask(clip, Some(RectMaskComponent::default()));
        element(&mut scene, clip, [0.0, 0.0, 2.0, 2.0]);
    }
    assert!(
        made(&mut renderer, &mut frames, &scene) > base,
        "a new buffer"
    );
    assert_eq!(made(&mut renderer, &mut frames, &scene), base, "then kept");

    // A new view size re-makes the Mask's texture, and the groups binding it.
    frames.view.resize(&renderer.device, SIZE / 2, SIZE / 2);
    assert!(
        made(&mut renderer, &mut frames, &scene) > base,
        "a new mask target"
    );
    assert_eq!(made(&mut renderer, &mut frames, &scene), base, "then kept");

    // A group no frame asks for any more is dropped, with what it binds.
    let held = frames.view.ui.effects.group_count();
    scene.world.set_mask(mask, None);
    made(&mut renderer, &mut frames, &scene);
    made(&mut renderer, &mut frames, &scene);
    assert!(
        frames.view.ui.effects.group_count() < held,
        "the removed Mask's group"
    );
}
