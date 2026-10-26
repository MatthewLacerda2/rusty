//! `Graphics.*` colour grading (#1440): white balance, the Lift / Gamma / Gain
//! wheels and the hue shift write the main camera's volume, clamped, read back what
//! they wrote, and read neutral without a volume.

use std::cell::RefCell;

use mlua::Lua;
use rusty::components::CameraComponent;
use rusty::scene::authoring::default_post_processing;
use rusty::scene::Scene;

/// Run `script` against `scene` with the `Graphics` namespace registered.
fn run(scene: Scene, script: &str) -> Scene {
    let lua = Lua::new();
    let (scene, graphics, app) = (RefCell::new(scene), RefCell::default(), RefCell::default());
    lua.scope(|s| {
        rusty::api::graphics::register(&lua, s, &scene, &graphics, &app).unwrap();
        lua.load(script).exec().unwrap();
        Ok(())
    })
    .unwrap();
    scene.into_inner()
}

#[test]
fn grading_knobs_round_trip_and_clamp() {
    let mut scene = Scene::new();
    let id = scene.add_entity("Camera".to_string());
    scene.world.set_camera(id, Some(CameraComponent::default()));
    scene
        .world
        .set_post_processing(id, Some(default_post_processing()));
    let scene = run(
        scene,
        r#"
        local t, n = Graphics.GetWhiteBalance()
        assert(t == 0 and n == 0, "neutral white balance")
        local r, g, b, o = Graphics.GetGainWheel()
        assert(r == 1 and g == 1 and b == 1 and o == 0, "neutral gain")
        Graphics.SetWhiteBalance(250, -30)
        Graphics.SetLiftWheel(0.2, 2, 0.4, -5)
        Graphics.SetGammaWheel(1, 1, 1, 0.25)
        Graphics.SetHueShift(-400)
        t, n = Graphics.GetWhiteBalance()
        assert(t == 100 and n == -30, "temperature clamps to 100")
        r, g, b, o = Graphics.GetLiftWheel()
        assert(g == 1 and o == -1, "colour and offset clamp")
        assert(Graphics.GetHueShift() == -180)
    "#,
    );
    let vc = scene.world.post_processing(id).unwrap().clone();
    assert_eq!(vc.lift_gamma_gain.gamma.offset, 0.25);
    assert_eq!(vc.white_balance.tint, -30.0);
}

#[test]
fn without_a_volume_the_getters_read_neutral() {
    run(
        Scene::new(),
        r#"
        Graphics.SetHueShift(90)
        assert(Graphics.GetHueShift() == 0)
        local r, g, b, o = Graphics.GetLiftWheel()
        assert(r == 1 and o == 0)
    "#,
    );
}
