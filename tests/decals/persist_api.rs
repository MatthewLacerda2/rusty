//! Persistent decals through the `Decals` API (#1518): the `persist` opt, `Clear`
//! keeping the trail, and Stop dropping it anyway. What a load carries is
//! `scene::decal::persist`'s tests.

use std::cell::RefCell;
use std::rc::Rc;

use mlua::Lua;
use rusty::scene::Scene;

fn eval<T: mlua::FromLuaMulti>(scene: &RefCell<Scene>, script: &str) -> mlua::Result<T> {
    let lua = Lua::new();
    lua.scope(|scope| {
        rusty::api::decals::register(&lua, scope, scene).unwrap();
        lua.load(script).eval()
    })
}

#[test]
fn clear_with_persistent_false_keeps_the_trail() {
    let scene = RefCell::new(Scene::new());
    let (kept, count, after): (u32, u32, u32) = eval(
        &scene,
        "local hole = Decals.Spawn(0,0,0, 0,1,0, { persist = true })
         Decals.Spawn(1,0,0, 0,1,0, { persist = false })
         Decals.Spawn(2,0,0, 0,1,0)
         Decals.Clear({ persistent = false })
         local count = Decals.Count()
         Decals.Clear()
         return hole, count, Decals.Count()",
    )
    .unwrap();
    assert_eq!((count, after), (1, 0), "a plain Clear drops the trail too");
    assert!(kept > 0);
}

#[test]
fn an_unknown_clear_option_is_an_error() {
    let scene = RefCell::new(Scene::new());
    let err = eval::<()>(&scene, "Decals.Clear({ persist = false })").unwrap_err();
    assert!(err.to_string().contains("expected persistent"), "{err}");
}

#[test]
fn stop_drops_persistent_decals_with_the_rest() {
    let scene = Rc::new(RefCell::new(Scene::new()));
    let mut game = crate::play::playing(Rc::clone(&scene), 20.0);
    game.tick(1.0 / 60.0); // enter Play
    eval::<u32>(
        &scene,
        "return Decals.Spawn(0,0,0, 0,1,0, { persist = true })",
    )
    .unwrap();
    assert_eq!(scene.borrow().decals.len(), 1);
    game.set_playing(false);
    game.tick(1.0 / 60.0); // Stop
    assert!(
        scene.borrow().decals.is_empty(),
        "the snapshot restore clears"
    );
}
