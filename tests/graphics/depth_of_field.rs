//! `Graphics.*` depth of field (#1445): the mode and its knobs write the main
//! camera's volume, clamped, read back what they wrote, read Unity's defaults
//! without a volume, and an unknown mode is an error naming the valid ones. Its
//! knobs tween as `"Graphics.depth_of_field.<knob>"`.

use std::cell::RefCell;

use mlua::Lua;
use rusty::components::{CameraComponent, DofMode};
use rusty::scene::authoring::default_post_processing;
use rusty::scene::Scene;

/// Run `script` against `scene` with the `Graphics` namespace registered; the
/// script's error, if any.
fn run(scene: Scene, script: &str) -> (Scene, Option<String>) {
    let lua = Lua::new();
    let (scene, graphics, app) = (RefCell::new(scene), RefCell::default(), RefCell::default());
    let err = lua
        .scope(|s| {
            rusty::api::graphics::register(&lua, s, &scene, &graphics, &app).unwrap();
            Ok(lua.load(script).exec().err().map(|e| e.to_string()))
        })
        .unwrap();
    (scene.into_inner(), err)
}

fn camera_with_volume() -> (Scene, u32) {
    let mut scene = Scene::new();
    let id = scene.add_entity("Camera".to_string());
    scene.world.set_camera(id, Some(CameraComponent::default()));
    scene
        .world
        .set_post_processing(id, Some(default_post_processing()));
    (scene, id)
}

#[test]
fn the_mode_and_knobs_round_trip_and_clamp() {
    let (scene, id) = camera_with_volume();
    let (scene, err) = run(
        scene,
        r#"
        assert(Graphics.GetDepthOfFieldMode() == "Off")
        assert(Graphics.GetDepthOfFieldFocusDistance() == 10)
        Graphics.SetDepthOfFieldMode("Gaussian")
        Graphics.SetDepthOfFieldGaussianMaxRadius(0)
        Graphics.SetDepthOfFieldAperture(64)
        Graphics.SetDepthOfFieldFocusDistance(3.5)
        assert(Graphics.GetDepthOfFieldMode() == "Gaussian")
        assert(Graphics.GetDepthOfFieldGaussianMaxRadius() == 0.5)
        assert(Graphics.GetDepthOfFieldAperture() == 32)
        assert(Graphics.GetDepthOfFieldFocusDistance() == 3.5)
    "#,
    );
    assert_eq!(err, None);
    let dof = scene.world.post_processing(id).unwrap().depth_of_field;
    assert_eq!((dof.mode, dof.focus_distance), (DofMode::Gaussian, 3.5));
}

#[test]
fn an_unknown_mode_is_an_error_and_without_a_volume_the_getters_read_defaults() {
    let (scene, id) = camera_with_volume();
    let (scene, err) = run(scene, r#"Graphics.SetDepthOfFieldMode("Tilt")"#);
    let err = err.expect("an unknown mode raises");
    assert!(err.contains("Off, Gaussian, Bokeh"), "{err}");
    let mode = scene.world.post_processing(id).unwrap().depth_of_field.mode;
    assert_eq!(mode, DofMode::Off, "nothing written");
    let (_, err) = run(
        Scene::new(),
        r#"
        Graphics.SetDepthOfFieldAperture(2)
        assert(math.abs(Graphics.GetDepthOfFieldAperture() - 5.6) < 1e-5)
        assert(Graphics.GetDepthOfFieldFocalLength() == 50)
    "#,
    );
    assert_eq!(err, None);
}
