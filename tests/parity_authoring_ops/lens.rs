//! `Graphics.*` lens-effect bindings vs `authoring::rendering::post_processing`'s
//! lens ops (#1439): the same writes through both reach the same component.

use std::cell::RefCell;

use mlua::Lua;
use rusty::components::{CameraComponent, PostProcessingComponent};
use rusty::scene::authoring::{default_post_processing, rendering::post_processing as vc_ops};
use rusty::scene::Scene;

/// A camera with an active volume, and its id.
fn scene() -> (Scene, u32) {
    let mut scene = Scene::new();
    let id = scene.add_entity("Camera".to_string());
    scene.world.set_camera(id, Some(CameraComponent::default()));
    scene
        .world
        .set_post_processing(id, Some(default_post_processing()));
    (scene, id)
}

const SCRIPT: &str = r#"
Graphics.SetVignetteActive(true)
Graphics.SetVignetteIntensity(2)
Graphics.SetVignetteSmoothness(0.5)
Graphics.SetVignetteColor(0.2, 0.1, 9)
Graphics.SetVignetteCenter(-1, 0.4)
Graphics.SetChromaticAberrationActive(true)
Graphics.SetChromaticAberrationIntensity(0.7)
Graphics.SetFilmGrainActive(true)
Graphics.SetFilmGrainIntensity(0.6)
Graphics.SetFilmGrainResponse(5)
"#;

fn apply_ops(vc: &mut PostProcessingComponent) {
    vc_ops::lens::vignette::set_active(vc, true);
    vc_ops::lens::vignette::set_intensity(vc, 2.0);
    vc_ops::lens::vignette::set_smoothness(vc, 0.5);
    vc_ops::lens::vignette::set_color(vc, [0.2, 0.1, 9.0]);
    vc_ops::lens::vignette::set_center(vc, [-1.0, 0.4]);
    vc_ops::lens::chromatic_aberration::set_active(vc, true);
    vc_ops::lens::chromatic_aberration::set_intensity(vc, 0.7);
    vc_ops::lens::film_grain::set_active(vc, true);
    vc_ops::lens::film_grain::set_intensity(vc, 0.6);
    vc_ops::lens::film_grain::set_response(vc, 5.0);
}

#[test]
fn lens_api_and_shared_op_converge() {
    let (lua_scene, id) = scene();
    let lua_scene = RefCell::new(lua_scene);
    let (graphics, app) = (RefCell::default(), RefCell::default());
    let lua = Lua::new();
    lua.scope(|s| {
        rusty::api::graphics::register(&lua, s, &lua_scene, &graphics, &app).unwrap();
        lua.load(SCRIPT).exec()
    })
    .unwrap();
    let (mut op_scene, op_id) = scene();
    apply_ops(&mut op_scene.world.post_processing_mut(op_id).unwrap());
    let lens = |s: &Scene, id| {
        let vc = s.world.post_processing(id).unwrap();
        (vc.vignette, vc.chromatic_aberration, vc.film_grain)
    };
    let got = lens(&lua_scene.borrow(), id);
    assert_eq!(got, lens(&op_scene, op_id));
    assert_eq!(got.0.intensity, 1.0, "single-sourced clamp");
    assert_eq!(got.0.center, [0.0, 0.4], "single-sourced clamp");
    assert_eq!(got.2.response, 1.0, "single-sourced clamp");
}
