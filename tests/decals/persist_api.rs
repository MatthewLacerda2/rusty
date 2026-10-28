//! Persistent decals through the `Decals` API (#1518): the `persist` opt, `Clear`
//! keeping the trail, and Stop reverting to the edit-mode decals anyway. What a load carries is
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
fn persist_and_persistent_take_a_boolean_only() {
    let scene = RefCell::new(Scene::new());
    for script in [
        "Decals.Spawn(0,0,0, 0,1,0, { persist = \"false\" })",
        "Decals.Spawn(0,0,0, 0,1,0, { persist = 0 })",
        "Decals.Clear({ persistent = \"false\" })",
    ] {
        let err = eval::<()>(&scene, script).unwrap_err().to_string();
        assert!(err.contains("must be true or false"), "{script}: {err}");
    }
    assert_eq!(scene.borrow().decals.len(), 0, "nothing stamped");
}

#[test]
fn stop_puts_back_the_edit_mode_decals_and_drops_plays() {
    let scene = Rc::new(RefCell::new(Scene::new()));
    let wall = scene.borrow_mut().add_entity("Wall".into());
    let graffiti: u32 = eval(
        &scene,
        &format!("return Decals.Spawn(0,0,0, 0,1,0, {{ owner = {wall} }})"),
    )
    .unwrap();
    let mut game = crate::play::playing(Rc::clone(&scene), 20.0);
    game.tick(1.0 / 60.0); // enter Play
    let hole: u32 = eval(
        &scene,
        "return Decals.Spawn(0,0,0, 0,1,0, { persist = true })",
    )
    .unwrap();
    assert_eq!(scene.borrow().decals.len(), 2);
    game.set_playing(false);
    game.tick(1.0 / 60.0); // Stop
    let s = scene.borrow();
    let kept: Vec<_> = s
        .decals
        .iter()
        .map(|d| (d.id, d.owner.map(|o| o.entity)))
        .collect();
    assert_eq!(
        kept,
        [(graffiti, Some(wall))],
        "the edit decal, on its owner"
    );
    assert!(hole > graffiti);
}
