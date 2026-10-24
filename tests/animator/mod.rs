mod graph_api;
mod layers_api;
mod parameters;
mod speed_api;

use std::cell::RefCell;

use mlua::Lua;
use rusty::components::AnimatorComponent;
use rusty::scene::Scene;
use rusty::scripting::ConsoleLogs;

/// A scene with one entity, "Rig", carrying a default Animator.
fn scene_with_animator() -> (RefCell<Scene>, u32) {
    let mut scene = Scene::new();
    let id = scene.add_entity("Rig".to_string());
    scene
        .world
        .set_animator(id, Some(AnimatorComponent::default()));
    (RefCell::new(scene), id)
}

/// Register the `Animator` namespace against `scene`, run `script`, return its
/// value rendered through Lua's `tostring` (the parentheses adjust a zero-value
/// call — a plain setter — to `nil`).
fn run(scene: &RefCell<Scene>, script: &str) -> String {
    let console = RefCell::new(ConsoleLogs::new());
    let lua = Lua::new();
    lua.scope(|s| {
        rusty::api::animator::register(&lua, s, scene, &console).unwrap();
        lua.load(format!("return tostring(({script}))")).eval()
    })
    .unwrap()
}
