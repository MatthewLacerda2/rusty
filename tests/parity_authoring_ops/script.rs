//! `Scene.AddScript` vs `authoring::attach_script` (#998): the editor's script list
//! and the Lua verb append the same slot.

use std::cell::RefCell;

use mlua::Lua;
use rusty::scene::authoring::attach_script;
use rusty::scene::Scene;

const SCRIPT: &str = "assets/scripts/bot.lua";

#[test]
fn add_script_and_shared_op_converge() {
    let scene = RefCell::new(Scene::new());
    let (lua_id, op_id) = {
        let mut s = scene.borrow_mut();
        (s.add_entity("Lua".into()), s.add_entity("Op".into()))
    };
    attach_script(
        &mut scene.borrow_mut().world,
        op_id,
        SCRIPT,
        Default::default(),
    );

    let lua = Lua::new();
    let (path, playing) = (RefCell::new(None), RefCell::new(false));
    lua.scope(|s| {
        rusty::api::scene::register(&lua, s, &scene, &path, &playing).unwrap();
        let added: bool = lua
            .load(format!("return Scene.AddScript({lua_id}, '{SCRIPT}')"))
            .eval()?;
        assert!(added);
        let missing = format!("Scene.AddScript({lua_id}, 'assets/scripts/nope.lua')");
        assert!(lua.load(missing).exec().is_err(), "a missing file errors");
        Ok(())
    })
    .unwrap();

    let s = scene.borrow();
    let slots = |id| format!("{:?}", *s.world.scripts(id).unwrap());
    assert_eq!(slots(lua_id), slots(op_id));
}
