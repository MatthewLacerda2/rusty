//! Runtime postfx params pixel test (#671): a script's `Graphics.SetPostParam`
//! drives a baked damage vignette's intensity, and the corner of the frame follows
//! it with no re-bake — the module's files stay untouched; only the volume's value
//! (a uniform write) changes between frames. Its own files, not the process-wide
//! bake generation, which a neighbouring test's bake moves (#794).

use std::cell::RefCell;

use mlua::Lua;
use rusty::core::settings::GraphicsSettings;
use rusty::dev::capture::CaptureHost;
use rusty::scene::Scene;
use rusty::shadergen::{bake_recipe, engine_shader_dir, ShaderRecipe, DEFAULT_OUT_DIR};

use super::super::fog::scene::{shot, srgb8, wall_scene};

const NAME: &str = "t671_hurt";

/// A grey wall whose volume runs a red damage vignette that does not pulse.
fn scene() -> RefCell<Scene> {
    let json = format!(
        r#"{{"pass":"postfx","name":"{NAME}","blocks":[{{"id":"damage_vignette","params":{{"color":[1,0,0]}}}}]}}"#
    );
    bake_recipe(
        &rusty::api::recipe_from_json_str::<ShaderRecipe>(&json).unwrap(),
        engine_shader_dir(),
        DEFAULT_OUT_DIR,
    )
    .expect("bake");
    let mut scene = wall_scene(2.0, [0.5, 0.5, 0.5]);
    let id = scene.world.ids_with_post_processing()[0];
    scene
        .world
        .post_processing_mut(id)
        .unwrap()
        .custom_effects
        .names = vec![NAME.into()];
    RefCell::new(scene)
}

/// When the module and its params sidecar were last written: a bake rewrites both.
fn written() -> impl PartialEq + std::fmt::Debug {
    ["wgsl", "params.json", "recipe.json"].map(|ext| {
        let path = format!("{DEFAULT_OUT_DIR}/{NAME}.{ext}");
        std::fs::metadata(path).and_then(|m| m.modified()).ok()
    })
}

/// Run `script` against the `Graphics` namespace.
fn run(scene: &RefCell<Scene>, script: &str) {
    let (lua, quality) = (Lua::new(), RefCell::new(GraphicsSettings::default()));
    let app = RefCell::default();
    lua.scope(|s| {
        rusty::api::graphics::register(&lua, s, scene, &quality, &app).unwrap();
        lua.load(script).exec()
    })
    .expect("script");
}

/// Set the intensity from a script, render, and return the top-left corner pixel.
fn corner_at(host: &mut CaptureHost, scene: &RefCell<Scene>, intensity: f32) -> Option<[u8; 3]> {
    run(
        scene,
        &format!(r#"Graphics.SetPostParam("damage_vignette.intensity", {intensity})"#),
    );
    let img = shot(
        host,
        &scene.borrow(),
        &format!("pfx671_{}", (intensity * 10.0) as u32),
    )?;
    Some(img.get_pixel(0, 0).0)
}

#[test]
fn a_script_set_intensity_moves_the_vignette_edge_without_a_rebake() {
    let scene = scene();
    let mut host = CaptureHost::new();
    let baked = written();
    let Some(off) = corner_at(&mut host, &scene, 0.0) else {
        return;
    };
    let half = corner_at(&mut host, &scene, 0.5).unwrap();
    let full = corner_at(&mut host, &scene, 1.0).unwrap();
    let after = written();
    for ext in ["wgsl", "params.json", "recipe.json"] {
        std::fs::remove_file(format!("{DEFAULT_OUT_DIR}/{NAME}.{ext}")).ok();
    }

    let grey = srgb8(0.5);
    assert!(
        off.iter().all(|&c| (c as i32 - grey).abs() <= 3),
        "intensity 0 is the plain wall: {off:?}"
    );
    assert!(
        full[0] >= 230 && full[1] <= 60 && full[2] <= 60,
        "intensity 1 is red at the edge: {full:?}"
    );
    assert!(
        half[1] < off[1] && half[1] > full[1],
        "0.5 sits between: {half:?}"
    );
    assert_eq!(after, baked, "a param write never re-bakes");
}
