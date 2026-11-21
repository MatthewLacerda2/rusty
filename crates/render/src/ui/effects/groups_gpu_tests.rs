use rusty_components::RectMaskComponent;
use rusty_scene::Scene;

use super::gpu_fixture::{element, minimap, Frames, SIZE};
use crate::Renderer;

fn made(renderer: &mut Renderer, frames: &mut Frames, scene: &Scene) -> u32 {
    frames.frame(renderer, scene);
    renderer.frame_counters.ops.bind_groups_created
}

#[test]
fn gpu_a_still_hud_makes_no_batch_group_and_a_replaced_resource_remakes_one() {
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(SIZE, SIZE) else {
        return;
    };
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

    frames.view.resize(&renderer.device, SIZE / 2, SIZE / 2);
    assert!(
        made(&mut renderer, &mut frames, &scene) > base,
        "a new mask target"
    );
    assert_eq!(made(&mut renderer, &mut frames, &scene), base, "then kept");

    let held = frames.view.ui.effects.group_count();
    scene.world.set_mask(mask, None);
    made(&mut renderer, &mut frames, &scene);
    made(&mut renderer, &mut frames, &scene);
    assert!(
        frames.view.ui.effects.group_count() < held,
        "the removed Mask's group"
    );
}
