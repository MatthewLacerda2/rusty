//! `Graphics.SetPostParam` / `GetPostParam` (#671): values land on the active volume,
//! read back (defaults included), a name the volume's effects don't expose at runtime
//! is an error naming it, and with no volume a set errors while a get answers with the
//! catalog default.

use std::cell::RefCell;

use mlua::Lua;

use crate::scene::authoring::defaults;
use crate::scene::Scene;
use crate::shadergen::{bake_recipe, engine_shader_dir, ShaderRecipe, DEFAULT_OUT_DIR};

/// A baked damage-vignette postfx module, unique to this test and run (tests share
/// a process under `cargo test`); removed on drop.
struct Baked(String);

impl Baked {
    fn new(tag: &str) -> Self {
        let name = format!("test_post_params_{tag}_{}", std::process::id());
        let json =
            format!(r#"{{"pass":"postfx","name":"{name}","blocks":[{{"id":"damage_vignette"}}]}}"#);
        let lua = Lua::new();
        let recipe: ShaderRecipe = crate::api::lua_json::recipe_from_lua(&mlua::Value::String(
            lua.create_string(&json).unwrap(),
        ))
        .unwrap();
        bake_recipe(&recipe, engine_shader_dir(), DEFAULT_OUT_DIR).unwrap();
        Self(name)
    }
}

impl Drop for Baked {
    fn drop(&mut self) {
        for ext in ["wgsl", "params.json", "recipe.json"] {
            let _ = std::fs::remove_file(format!("{DEFAULT_OUT_DIR}/{}.{ext}", self.0));
        }
    }
}

/// Run `script` against the `Graphics` namespace, returning its error if any.
fn run(scene: &RefCell<Scene>, script: &str) -> Result<(), String> {
    let (lua, graphics) = (Lua::new(), RefCell::new(Default::default()));
    let app = RefCell::default();
    lua.scope(|s| {
        crate::api::graphics::register(&lua, s, scene, &graphics, &app).unwrap();
        lua.load(script).exec()
    })
    .map_err(|e| e.to_string())
}

/// A scene whose active volume runs `effect`.
fn scene_running(effect: &str) -> RefCell<Scene> {
    let mut scene = Scene::new();
    let id = scene.add_entity("Volume".into());
    let mut vc = defaults::default_post_processing();
    vc.custom_effects.names = vec![effect.to_owned()];
    scene.world.set_post_processing(id, Some(vc));
    // `Graphics.*` edits the main camera's volume (#1434).
    scene.world.set_camera(id, Some(defaults::default_camera()));
    RefCell::new(scene)
}

#[test]
fn values_land_on_the_volume_and_read_back() {
    let baked = Baked::new("values");
    let scene = scene_running(&baked.0);
    run(
        &scene,
        r#"assert(Graphics.GetPostParam("damage_vignette.intensity") == 0.5)
           Graphics.SetPostParam("damage_vignette.intensity", 0.75)
           Graphics.SetPostParam("damage_vignette.0.color", {1, 0, 0})
           assert(Graphics.GetPostParam("damage_vignette.intensity") == 0.75)
           assert(Graphics.GetPostParam("damage_vignette.0.color")[1] == 1)"#,
    )
    .unwrap();
    let scene = scene.borrow();
    let id = scene.world.ids_with_post_processing()[0];
    let vc = scene.world.post_processing(id).unwrap();
    assert_eq!(
        vc.custom_effects.params["damage_vignette.intensity"],
        [0.75]
    );
    assert_eq!(
        vc.custom_effects.params["damage_vignette.0.color"],
        [1.0, 0.0, 0.0]
    );
}

#[test]
fn a_param_the_effects_do_not_expose_is_an_error_naming_it() {
    let baked = Baked::new("errors");
    let scene = scene_running(&baked.0);
    for (call, want) in [
        (r#""damage_vignette.pulse_speed", 1"#, "is baked"),
        (r#""radial_blur.strength", 1"#, "damage_vignette.intensity"),
        (r#""damage_vignette.color", {1, 0}"#, "takes 3 number(s)"),
        (r#""damage_vignette.intensity", "x""#, "number or an array"),
    ] {
        let err = run(&scene, &format!("Graphics.SetPostParam({call})")).unwrap_err();
        assert!(err.contains(want), "{call}: {err}");
    }
}

#[test]
fn without_a_volume_a_set_errors_and_a_get_is_the_catalog_default() {
    let scene = RefCell::new(Scene::new());
    let err = run(
        &scene,
        r#"Graphics.SetPostParam("damage_vignette.intensity", 1)"#,
    );
    assert!(err
        .unwrap_err()
        .contains("no active post-processing volume"));
    run(
        &scene,
        r#"assert(Graphics.GetPostParam("damage_vignette.intensity") == 0.5)
           assert(not pcall(Graphics.GetPostParam, "posterize.levels"))"#,
    )
    .unwrap();
}
