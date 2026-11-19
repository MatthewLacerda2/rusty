//! GPU tests for a still HUD's Masks (#2354): a Mask's coverage texture is drawn
//! again only when an input it depends on changed, each input in turn, and its
//! picture is the same whether it was drawn or kept.

use glam::{Vec2, Vec4};

use super::gpu_fixture::{element, minimap, Frames, SIZE};
use rusty_components::{MaskComponent, RectMaskComponent};

#[test]
fn gpu_a_still_mask_is_drawn_once_and_again_only_when_an_input_changes() {
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(SIZE, SIZE) else {
        return;
    };
    let (mut scene, panel, mask, map) = minimap();
    let mut frames = Frames::new(&renderer);
    let (passes, first) = frames.frame(&mut renderer, &scene);
    assert_eq!(passes, 1, "the first frame draws the Mask");
    let (passes, still) = frames.frame(&mut renderer, &scene);
    assert_eq!(passes, 0, "a still Mask is kept");
    assert_eq!(still, first, "and shows what it drew");
    assert_eq!(frames.view.ui.effects.uniform_uploads, 0, "its slots too");

    // The minimap's map turns under its still frame: the Mask stays.
    scene.world.image_mut(map).expect("map").color = Vec4::new(1.0, 0.0, 0.0, 1.0);
    let (passes, red) = frames.frame(&mut renderer, &scene);
    assert_eq!(passes, 0, "only the masked content changed");
    assert_ne!(red, first, "and it is drawn red");

    // Each input of the Mask's draw, in turn.
    scene.world.shape_mut(mask).expect("shape").color.w = 0.5;
    assert_eq!(frames.frame(&mut renderer, &scene).0, 1, "its graphic");
    let mut rt = scene.world.rect_transform_mut(mask).expect("rect");
    rt.size_delta = Vec2::splat(40.0);
    drop(rt);
    assert_eq!(frames.frame(&mut renderer, &scene).0, 1, "its rect");
    let feather = RectMaskComponent {
        feather: 8.0,
        ..Default::default()
    };
    scene.world.set_rect_mask(panel, Some(feather));
    assert_eq!(
        frames.frame(&mut renderer, &scene).0,
        1,
        "its slot (the clip above)"
    );
    assert_eq!(
        frames.view.ui.effects.uniform_uploads, 1,
        "a slot moved: uploaded"
    );
    frames.view.resize(&renderer.device, SIZE / 2, SIZE / 2);
    assert_eq!(frames.frame(&mut renderer, &scene).0, 1, "the view's size");
    assert_eq!(frames.frame(&mut renderer, &scene).0, 0, "and still again");
}

#[test]
fn gpu_a_redrawn_parent_mask_redraws_its_children_and_a_live_source_every_frame() {
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(SIZE, SIZE) else {
        return;
    };
    let (mut scene, _, mask, map) = minimap();
    scene.world.set_mask(map, Some(MaskComponent::default()));
    element(&mut scene, map, [0.0, 0.0, 16.0, 16.0]);
    let mut frames = Frames::new(&renderer);
    assert_eq!(frames.frame(&mut renderer, &scene).0, 2, "both drawn first");
    assert_eq!(frames.frame(&mut renderer, &scene).0, 0, "both kept");
    scene.world.shape_mut(mask).expect("shape").color.w = 0.5;
    assert_eq!(
        frames.frame(&mut renderer, &scene).0,
        2,
        "the child samples it"
    );
    scene.world.image_mut(map).expect("map").color.w = 0.5;
    assert_eq!(frames.frame(&mut renderer, &scene).0, 1, "the child alone");

    // A render texture changes in place: a Mask showing one is drawn each frame.
    scene.world.image_mut(map).expect("map").texture = Some("rt:live".into());
    for _ in 0..2 {
        assert_eq!(frames.frame(&mut renderer, &scene).0, 1, "the live source");
    }
}
