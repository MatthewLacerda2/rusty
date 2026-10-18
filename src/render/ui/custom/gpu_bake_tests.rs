//! Custom ui shaders through the real UI pass (#427), how a shader is found and
//! rebuilt: a missing or non-ui shader falls back to the standard one, a runtime
//! param is a buffer write (no rebuild), and a re-bake is picked up next frame while
//! another shader's bake rebuilds nothing (#794). How a shaded graphic draws is
//! `gpu_draw_tests`. Skips with no adapter.

use std::cell::RefCell;

use super::fixture::{lua, scene, shot, Baked, RES};
use crate::render::test_gpu::headless_or_skip;
use crate::shadergen::recipe::{BlockSel, PassKind, ShaderRecipe};
use crate::shadergen::{bake_recipe, engine_shader_dir, DEFAULT_OUT_DIR};

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
    bake_recipe(&recipe, engine_shader_dir(), DEFAULT_OUT_DIR).unwrap();
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
