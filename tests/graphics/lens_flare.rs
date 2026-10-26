//! `Graphics.*` lens flare (#1443): every `SetLensFlare*` writes the main camera's
//! volume, clamped, and its `Get*` reads back what it wrote, the hand-written ghost
//! count and tint included; without a volume the getters read the defaults.

use std::cell::RefCell;

use mlua::Lua;
use rusty::components::CameraComponent;
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
fn every_lens_flare_setter_round_trips_and_clamps() {
    let (scene, id) = camera_with_volume();
    let (scene, err) = run(
        scene,
        r#"
        assert(Graphics.GetLensFlareActive() == false)
        Graphics.SetLensFlareActive(true)
        Graphics.SetLensFlareIntensity(-2)
        Graphics.SetLensFlareGhostCount(99)
        Graphics.SetLensFlareGhostIntensity(1.5)
        Graphics.SetLensFlareHaloIntensity(0.25)
        Graphics.SetLensFlareStreakIntensity(2)
        Graphics.SetLensFlareTint(2, 0.5, -1)
        Graphics.SetLensFlareChromaticAberration(3)
        assert(Graphics.GetLensFlareActive() == true)
        assert(Graphics.GetLensFlareIntensity() == 0)
        assert(Graphics.GetLensFlareGhostCount() == 8)
        assert(Graphics.GetLensFlareGhostIntensity() == 1.5)
        assert(Graphics.GetLensFlareHaloIntensity() == 0.25)
        assert(Graphics.GetLensFlareStreakIntensity() == 2)
        local r, g, b = Graphics.GetLensFlareTint()
        assert(r == 1 and g == 0.5 and b == 0)
        assert(Graphics.GetLensFlareChromaticAberration() == 1)
    "#,
    );
    assert_eq!(err, None);
    let f = scene.world.post_processing(id).unwrap().lens_flare;
    assert!(f.active);
    assert_eq!((f.ghost_count, f.tint), (8, [1.0, 0.5, 0.0]));
}

#[test]
fn without_a_volume_the_getters_read_the_defaults() {
    let (_, err) = run(
        Scene::new(),
        r#"
        Graphics.SetLensFlareGhostCount(2)
        Graphics.SetLensFlareTint(0, 0, 0)
        assert(Graphics.GetLensFlareActive() == false)
        assert(Graphics.GetLensFlareGhostCount() == 4)
        local r, g, b = Graphics.GetLensFlareTint()
        assert(r == 1 and g == 1 and b == 1)
        assert(Graphics.GetLensFlareHaloIntensity() == 0.5)
    "#,
    );
    assert_eq!(err, None);
}
