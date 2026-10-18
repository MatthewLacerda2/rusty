//! How a custom-shaded ui graphic draws through the real UI pass (#427): a RectMask
//! or a shaped mask still clips it, a shape draws through its shader, and a shaded
//! graphic gets its own batch while its text is one. Skips with no adapter.

use glam::Vec4;

use super::fixture::{element, scene, shade, shot, Baked, RES};
use crate::components::RectMaskComponent;
use crate::render::test_gpu::headless_or_skip;

#[test]
fn gpu_a_rect_mask_still_clips_a_custom_shaded_graphic() {
    let Some(mut renderer) = headless_or_skip(RES, RES) else {
        return;
    };
    let baked = Baked::new("clip", &[("hologram", &[("hue", 240.0), ("flicker", 0.0)])]);
    let (mut scene, _, image) = scene(None);
    scene.world.set_image(image, None);
    scene
        .world
        .set_rect_mask(image, Some(RectMaskComponent::default()));
    let child = element(&mut scene, image, [8.0, 8.0, 40.0, 40.0], Some(Vec4::ONE));
    shade(&mut scene, child, Some(&baked.0));
    let shot = shot(&mut renderer, &mut None, &scene);
    let inside = shot.px(40, 40);
    assert!(
        inside[2] > inside[0].saturating_add(60),
        "shaded inside: {inside:?}"
    );
    assert!(
        shot.px(56, 56)[..3].iter().all(|&c| c < 15),
        "cut outside the mask"
    );
}

#[test]
fn gpu_a_shaded_graphic_is_its_own_batch_and_its_text_one_batch() {
    let Some(mut renderer) = headless_or_skip(RES, RES) else {
        return;
    };
    let baked = Baked::new("batch", &[("scanlines", &[])]);
    let (mut scene, root, _) = scene(Some(&baked.0));
    // Backdrop | shaded image | plain image: the shade breaks the run twice.
    element(&mut scene, root, [0.0, 0.0, 8.0, 8.0], Some(Vec4::ONE));
    let mut view = None;
    shot(&mut renderer, &mut view, &scene);
    assert_eq!(view.as_ref().unwrap().ui.last_batches(), 3);
    // A shaded label: every glyph shares the element's shade, so one draw.
    let label = element(&mut scene, root, [0.0, 48.0, 64.0, 16.0], None);
    let text = crate::components::TextComponent {
        text: "GLITCH".into(),
        shader: Some(crate::components::UiShader {
            name: baked.0.clone(),
            ..Default::default()
        }),
        ..Default::default()
    };
    scene.world.set_text(label, Some(text));
    shot(&mut renderer, &mut view, &scene);
    assert_eq!(view.as_ref().unwrap().ui.last_batches(), 4);
}

#[test]
fn gpu_a_shape_draws_through_its_shader_too() {
    let Some(mut renderer) = headless_or_skip(RES, RES) else {
        return;
    };
    let baked = Baked::new(
        "shape",
        &[("wipe", &[("progress", 0.5), ("softness", 0.01)])],
    );
    let (mut scene, _, id) = scene(None);
    scene.world.set_image(id, None);
    let shape = crate::components::ShapeComponent {
        shader: Some(crate::components::UiShader {
            name: baked.0.clone(),
            ..Default::default()
        }),
        ..Default::default()
    };
    scene.world.set_shape(id, Some(shape));
    let shot = shot(&mut renderer, &mut None, &scene);
    assert!(shot.px(20, 32)[0] > 240, "left half of the rect shows");
    assert!(shot.px(44, 32)[0] < 15, "right half wiped");
}

#[test]
fn gpu_a_shaped_mask_still_cuts_a_custom_shaded_graphic() {
    let Some(mut renderer) = headless_or_skip(RES, RES) else {
        return;
    };
    let baked = Baked::new("mask", &[("hologram", &[("hue", 240.0), ("flicker", 0.0)])]);
    let (mut scene, _, mask) = scene(None);
    // The mask is a hidden circle over RECT; its shaded child covers the whole screen.
    scene.world.set_image(mask, None);
    let circle = crate::components::ShapeComponent {
        kind: crate::components::ShapeKind::Ellipse,
        ..Default::default()
    };
    scene.world.set_shape(mask, Some(circle));
    let hidden = crate::components::MaskComponent {
        show_mask_graphic: false,
    };
    scene.world.set_mask(mask, Some(hidden));
    let child = element(
        &mut scene,
        mask,
        [-16.0, -16.0, 64.0, 64.0],
        Some(Vec4::ONE),
    );
    shade(&mut scene, child, Some(&baked.0));
    let shot = shot(&mut renderer, &mut None, &scene);
    let mid = shot.px(32, 32);
    assert!(
        mid[2] > mid[0].saturating_add(60),
        "shaded in the circle: {mid:?}"
    );
    assert!(
        shot.px(18, 18)[..3].iter().all(|&c| c < 15),
        "cut at the circle's corner"
    );
    assert!(
        shot.px(8, 32)[..3].iter().all(|&c| c < 15),
        "cut outside the mask"
    );
}
