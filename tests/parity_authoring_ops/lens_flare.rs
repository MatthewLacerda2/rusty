//! `Graphics.*` lens-flare bindings vs `authoring::rendering::post_processing`'s
//! lens-flare ops (#1443): the same writes through both reach the same component.

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
Graphics.SetLensFlareActive(true)
Graphics.SetLensFlareIntensity(1.7)
Graphics.SetLensFlareGhostCount(12)
Graphics.SetLensFlareGhostIntensity(-1)
Graphics.SetLensFlareHaloIntensity(0.3)
Graphics.SetLensFlareStreakIntensity(0.9)
Graphics.SetLensFlareTint(1.4, 0.6, 0.2)
Graphics.SetLensFlareChromaticAberration(0.75)
"#;

fn apply_ops(vc: &mut PostProcessingComponent) {
    let ops = vc_ops::lens_flare::EFFECT;
    assert_eq!(ops.name, "lens_flare");
    vc_ops::lens_flare::set_active(vc, true);
    vc_ops::lens_flare::set_intensity(vc, 1.7);
    vc_ops::lens_flare::set_ghost_count(vc, 12);
    vc_ops::lens_flare::set_ghost_intensity(vc, -1.0);
    vc_ops::lens_flare::set_halo_intensity(vc, 0.3);
    vc_ops::lens_flare::set_streak_intensity(vc, 0.9);
    vc_ops::lens_flare::set_tint(vc, [1.4, 0.6, 0.2]);
    vc_ops::lens_flare::set_chromatic_aberration(vc, 0.75);
}

#[test]
fn lens_flare_api_and_shared_op_converge() {
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
    let flare = |s: &Scene, id| s.world.post_processing(id).unwrap().lens_flare;
    let got = flare(&lua_scene.borrow(), id);
    assert_eq!(got, flare(&op_scene, op_id));
    assert_eq!((got.ghost_count, got.ghost_intensity), (8, 0.0), "clamped");
    assert_eq!(got.tint, [1.0, 0.6, 0.2]);
}
