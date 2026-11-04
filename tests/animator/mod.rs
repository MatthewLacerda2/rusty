mod graph_api;
mod layers_api;
mod parameters;
mod play_api;
mod speed_api;

use std::cell::RefCell;
use std::rc::Rc;

use mlua::Lua;
use rusty::api::bind;
use rusty::components::AnimatorComponent;
use rusty::scene::Scene;
use rusty::scripting::ConsoleLogs;

/// A scene with one entity, "Rig", carrying a default Animator.
fn scene_with_animator() -> (Rc<RefCell<Scene>>, u32) {
    let mut scene = Scene::new();
    let id = scene.add_entity("Rig".to_string());
    scene
        .world
        .set_animator(id, Some(AnimatorComponent::default()));
    (Rc::new(RefCell::new(scene)), id)
}

/// Register the `Animator` namespace against `scene`, run `script`, return its
/// value rendered through Lua's `tostring` (the parentheses adjust a zero-value
/// call — a plain setter — to `nil`).
fn run(scene: &Rc<RefCell<Scene>>, script: &str) -> String {
    let console = Rc::new(RefCell::new(ConsoleLogs::new()));
    let lua = Lua::new();
    crate::lua::attempt(|| {
        rusty::api::animator::register(&lua, bind(&lua, scene), bind(&lua, &console)).unwrap();
        lua.load(format!("return tostring(({script}))")).eval()
    })
    .unwrap()
}
