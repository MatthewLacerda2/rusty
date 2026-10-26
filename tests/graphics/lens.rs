//! `Graphics.*` lens effects (#1439): Vignette, Chromatic Aberration and Film Grain
//! write the main camera's volume, clamped, read back what they wrote, and read the
//! defaults without a volume.

use std::cell::RefCell;

use mlua::Lua;
use rusty::components::{CameraComponent, ChromaticAberration, FilmGrain, Vignette};
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
fn lens_knobs_round_trip_and_clamp() {
    let mut scene = Scene::new();
    let id = scene.add_entity("Camera".to_string());
    scene.world.set_camera(id, Some(CameraComponent::default()));
    let volume = Some(default_post_processing());
    scene.world.set_post_processing(id, volume);
    let scene = run(
        scene,
        r#"
        local function near(a, b) return math.abs(a - b) < 1e-6 end
        assert(not Graphics.GetVignetteActive() and not Graphics.GetFilmGrainActive())
        assert(not Graphics.GetChromaticAberrationActive(), "off by default")
        Graphics.SetVignetteActive(true)
        Graphics.SetVignetteIntensity(3)
        Graphics.SetVignetteSmoothness(0)
        Graphics.SetVignetteColor(0.5, 2, -1)
        Graphics.SetVignetteCenter(0.25, 0.75)
        Graphics.SetChromaticAberrationActive(true)
        Graphics.SetChromaticAberrationIntensity(0.5)
        Graphics.SetFilmGrainActive(true)
        Graphics.SetFilmGrainIntensity(-1)
        Graphics.SetFilmGrainResponse(0.4)
        assert(Graphics.GetVignetteActive() and Graphics.GetVignetteIntensity() == 1)
        assert(near(Graphics.GetVignetteSmoothness(), 0.01), "smoothness floor")
        local r, g, b = Graphics.GetVignetteColor()
        assert(r == 0.5 and g == 1 and b == 0, "colour clamps")
        local x, y = Graphics.GetVignetteCenter()
        assert(x == 0.25 and y == 0.75)
        assert(Graphics.GetChromaticAberrationIntensity() == 0.5)
        assert(Graphics.GetFilmGrainIntensity() == 0 and near(Graphics.GetFilmGrainResponse(), 0.4))
    "#,
    );
    let vc = scene.world.post_processing(id).unwrap();
    assert!(vc.vignette.active && vc.chromatic_aberration.active && vc.film_grain.active);
    let json = serde_json::to_string(&*vc).unwrap();
    let back: rusty::components::PostProcessingComponent = serde_json::from_str(&json).unwrap();
    assert_eq!(
        (back.vignette, back.chromatic_aberration, back.film_grain),
        (vc.vignette, vc.chromatic_aberration, vc.film_grain),
        "every knob survives a save and load"
    );
}

#[test]
fn without_a_volume_the_getters_report_the_defaults() {
    let lua = Lua::new();
    let (scene, graphics, app) = (
        RefCell::new(Scene::new()),
        RefCell::default(),
        RefCell::default(),
    );
    let got: (bool, f32, f32, f32, f32, f32, f32, f32) = lua
        .scope(|s| {
            rusty::api::graphics::register(&lua, s, &scene, &graphics, &app).unwrap();
            lua.load(
                r#"Graphics.SetVignetteActive(true)
                local x, y = Graphics.GetVignetteCenter()
                return Graphics.GetVignetteActive(), Graphics.GetVignetteIntensity(),
                    Graphics.GetVignetteSmoothness(), x, y,
                    Graphics.GetChromaticAberrationIntensity(),
                    Graphics.GetFilmGrainIntensity(), Graphics.GetFilmGrainResponse()"#,
            )
            .eval()
        })
        .unwrap();
    let (v, ca, g) = (
        Vignette::default(),
        ChromaticAberration::default(),
        FilmGrain::default(),
    );
    let want = (
        false,
        v.intensity,
        v.smoothness,
        v.center[0],
        v.center[1],
        ca.intensity,
        g.intensity,
        g.response,
    );
    assert_eq!(got, want, "no volume to write; the defaults read back");
}
