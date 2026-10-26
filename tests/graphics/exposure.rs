//! `Graphics.*` exposure (#1444): the mode and the automatic knobs write the main
//! camera's volume, clamped, and `GetExposure` reads the screen's adapted EV in
//! `Automatic`, the fixed one otherwise.

use std::cell::RefCell;

use mlua::Lua;
use rusty::components::{CameraComponent, ExposureMode};
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

fn with_volume() -> (Scene, u32) {
    let mut scene = Scene::new();
    let id = scene.add_entity("Camera".to_string());
    scene.world.set_camera(id, Some(CameraComponent::default()));
    scene
        .world
        .set_post_processing(id, Some(default_post_processing()));
    (scene, id)
}

#[test]
fn exposure_knobs_round_trip_and_clamp() {
    let (scene, id) = with_volume();
    let scene = run(
        scene,
        r#"
        assert(Graphics.GetExposureMode() == "Fixed", "a new volume is Fixed")
        local lo, hi = Graphics.GetHistogramPercent()
        assert(lo == 10 and hi == 90)
        Graphics.SetExposureMode("automatic")
        Graphics.SetExposureLimits(-3, -20)
        Graphics.SetExposureCompensation(1.5)
        Graphics.SetAdaptationSpeed(-1, 2)
        Graphics.SetHistogramPercent(50, 120)
        assert(Graphics.GetExposureMode() == "Automatic")
        local min, max = Graphics.GetExposureLimits()
        assert(min == -3 and max == -3, "max floors at min")
        assert(Graphics.GetExposureCompensation() == 1.5)
        local up, down = Graphics.GetAdaptationSpeed()
        assert(up == 0 and down == 2, "a negative speed clamps to 0")
        lo, hi = Graphics.GetHistogramPercent()
        assert(lo == 50 and hi == 100)
        Graphics.SetExposureMode("nonsense")
        assert(Graphics.GetExposureMode() == "Automatic", "an unknown name is ignored")
    "#,
    );
    let vc = scene.world.post_processing(id).unwrap().clone();
    assert_eq!(vc.exposure_mode, ExposureMode::Automatic);
    assert_eq!(vc.auto_exposure.compensation, 1.5);
}

#[test]
fn get_exposure_reads_the_adapted_ev_only_in_automatic() {
    let (mut scene, _) = with_volume();
    scene.screen_exposure = Some(2.5);
    run(
        scene,
        r#"
        Graphics.SetExposure(-1)
        assert(Graphics.GetExposure() == -1, "Fixed reads the fixed EV")
        Graphics.SetExposureMode("Automatic")
        assert(Graphics.GetExposure() == 2.5, "Automatic reads the screen's")
    "#,
    );
    let (scene, _) = with_volume();
    run(
        scene,
        r#"
        Graphics.SetExposure(0.75)
        Graphics.SetExposureMode("Automatic")
        assert(Graphics.GetExposure() == 0.75, "nothing metered yet")
    "#,
    );
}
