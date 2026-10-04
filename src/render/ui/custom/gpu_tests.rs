//! Custom ui shaders through the real UI pass (#427): a missing or non-ui shader
//! falls back to the standard one, a runtime param is a buffer write (no rebuild), a
//! re-bake is picked up next frame while another shader's bake rebuilds nothing
//! (#794), a RectMask still clips a custom-shaded graphic, and a shaded graphic gets
//! its own batch. Skips with no adapter.

use std::cell::RefCell;

use glam::Vec4;

use super::fixture::{element, lua, scene, shade, shot, Baked, RES};
use crate::components::RectMaskComponent;
use crate::render::test_gpu::headless_or_skip;
use crate::shadergen::recipe::{BlockSel, PassKind, ShaderRecipe};
use crate::shadergen::{bake_recipe, DEFAULT_OUT_DIR, ENGINE_SHADER_DIR};

#[test]
fn gpu_a_missing_or_non_ui_shader_draws_with_the_standard_one() {
    let Some(mut renderer) = headless_or_skip(RES, RES) else {
        return;
    };
    let surface = format!("test_ui_surface_{}", std::process::id());
    let recipe = ShaderRecipe {
        pass: PassKind::Surface,
        name: surface.clone(),
        blocks: vec![BlockSel {
            id: "toon_ramp".into(),
            params: Default::default(),
        }],
    };
    bake_recipe(&recipe, ENGINE_SHADER_DIR, DEFAULT_OUT_DIR).unwrap();
    let _cleanup = Baked(surface.clone());
    for name in ["no_such_ui_shader", surface.as_str()] {
        let (scene, _, _) = scene(Some(name));
        let px = shot(&mut renderer, &mut None, &scene).px(32, 32);
        assert!(
            px[..3].iter().all(|&c| c > 240),
            "{name}: white, got {px:?}"
        );
    }
}

#[test]
fn gpu_a_runtime_param_redraws_without_a_rebuild() {
    let Some(mut renderer) = headless_or_skip(RES, RES) else {
        return;
    };
    let baked = Baked::new("param", &[("wipe", &[("softness", 0.01)])]);
    let (scene, _, id) = scene(Some(&baked.0));
    let scene = RefCell::new(scene);
    let mut view = None;
    assert!(shot(&mut renderer, &mut view, &scene.borrow()).px(44, 32)[0] > 240);
    let builds = renderer.ui_renderer.shaders.builds();
    lua(
        &scene,
        &format!("UI.SetShaderParam({id}, 'wipe.progress', 0.25)"),
    );
    lua(
        &scene,
        &format!("assert(UI.GetShaderParam({id}, 'wipe.progress') == 0.25)"),
    );
    let after = shot(&mut renderer, &mut view, &scene.borrow());
    assert!(after.px(44, 32)[0] < 15, "the right side wiped away");
    assert!(after.px(18, 32)[0] > 240, "the left quarter still shows");
    assert_eq!(renderer.ui_renderer.shaders.builds(), builds, "no rebuild");
}

#[test]
fn gpu_a_rebake_is_picked_up_next_frame() {
    let Some(mut renderer) = headless_or_skip(RES, RES) else {
        return;
    };
    let hidden = Baked::new("rebake", &[("wipe", &[("progress", 0.0)])]);
    let (scene, _, _) = scene(Some(&hidden.0));
    let mut view = None;
    assert!(shot(&mut renderer, &mut view, &scene).px(32, 32)[0] < 15);
    let _shown = Baked::new("rebake", &[("wipe", &[("progress", 1.0)])]);
    assert!(shot(&mut renderer, &mut view, &scene).px(32, 32)[0] > 240);
}

#[test]
fn gpu_a_bake_of_another_shader_rebuilds_no_variant() {
    // #794: a bake re-reads every variant's module and drops only a changed one, so
    // a neighbouring test's bake under one-process `cargo test` rebuilds nothing here.
    let Some(mut renderer) = headless_or_skip(RES, RES) else {
        return;
    };
    let kept = Baked::new("kept", &[("wipe", &[("progress", 1.0)])]);
    let (scene, _, _) = scene(Some(&kept.0));
    let mut view = None;
    assert!(shot(&mut renderer, &mut view, &scene).px(32, 32)[0] > 240);
    let builds = renderer.ui_renderer.shaders.builds();
    let _unrelated = Baked::new("unrelated", &[("scanlines", &[])]);
    assert!(shot(&mut renderer, &mut view, &scene).px(32, 32)[0] > 240);
    assert!(renderer.ui_renderer.shaders.built(&kept.0));
    assert_eq!(renderer.ui_renderer.shaders.builds(), builds, "no rebuild");
}

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
