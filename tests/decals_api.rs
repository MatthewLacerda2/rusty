//! Integration coverage for the `Decals` scripting namespace (issue #350):
//! `Spawn` stamps a projector with the documented defaults and optional args,
//! `Count`/`Clear` track the live list, the FIFO cap evicts the oldest, and
//! malformed `Spawn` calls error instead of stamping garbage.

use std::cell::RefCell;

use mlua::Lua;
use rusty::scene::Scene;

fn empty_scene() -> RefCell<Scene> {
    RefCell::new(Scene::new())
}

#[test]
fn spawn_with_defaults_then_clear() {
    let lua = Lua::new();
    let scene = empty_scene();

    lua.scope(|scope| {
        rusty::api::decals::register(&lua, scope, &scene).unwrap();
        lua.load("Decals.Spawn(1, 2, 3, 0, 1, 0)").exec().unwrap();

        let n: u32 = lua.load("return Decals.Count()").eval().unwrap();
        assert_eq!(n, 1);
        {
            let s = scene.borrow();
            let d = &s.decals[0];
            assert_eq!(d.size.x, 0.5, "default stamp size");
            assert_eq!(d.size.z, 0.5, "default projection depth");
            assert_eq!(d.color, [1.0, 1.0, 1.0, 1.0], "default opaque white tint");
            assert!(d.texture.is_none(), "default checker texture");
        }

        lua.load("Decals.Clear()").exec().unwrap();
        let n: u32 = lua.load("return Decals.Count()").eval().unwrap();
        assert_eq!(n, 0);
        assert!(scene.borrow().decals.is_empty());
        Ok(())
    })
    .unwrap();
}

#[test]
fn spawn_honours_optional_size_texture_and_tint() {
    let lua = Lua::new();
    let scene = empty_scene();

    lua.scope(|scope| {
        rusty::api::decals::register(&lua, scope, &scene).unwrap();
        lua.load("Decals.Spawn(0, 0, 0, 0, 1, 0, 2.0, 'decals/scorch.png', 45, 1, 0, 0, 0.5)")
            .exec()
            .unwrap();

        let s = scene.borrow();
        let d = &s.decals[0];
        assert_eq!(d.size.x, 2.0, "explicit stamp size");
        assert_eq!(d.size.z, 2.0, "depth tracks size when size > default depth");
        assert_eq!(d.texture.as_deref(), Some("decals/scorch.png"));
        assert_eq!(d.color, [1.0, 0.0, 0.0, 0.5]);
        Ok(())
    })
    .unwrap();
}

#[test]
fn fifo_eviction_caps_the_live_list() {
    let lua = Lua::new();
    let scene = empty_scene();

    lua.scope(|scope| {
        rusty::api::decals::register(&lua, scope, &scene).unwrap();
        // Spawn well past the cap (256 today), each at x = i so survivors are
        // identifiable. The exact cap is the renderer's business; the API contract
        // under test is "bounded, oldest evicted first".
        lua.load("for i = 1, 400 do Decals.Spawn(i, 0, 0, 0, 1, 0) end")
            .exec()
            .unwrap();

        let n: u32 = lua.load("return Decals.Count()").eval().unwrap();
        assert!(n < 400, "the live list is bounded");
        // FIFO: with n survivors of 400, the first remaining is the (400 - n + 1)th.
        let first_x = scene.borrow().decals[0].position.x;
        assert_eq!(first_x, (400 - n + 1) as f32, "oldest decals evicted first");
        Ok(())
    })
    .unwrap();
}

#[test]
fn spawn_rejects_missing_or_non_numeric_required_args() {
    let lua = Lua::new();
    let scene = empty_scene();

    lua.scope(|scope| {
        rusty::api::decals::register(&lua, scope, &scene).unwrap();
        assert!(
            lua.load("Decals.Spawn(1, 2, 3)").exec().is_err(),
            "fewer than the 6 required point+normal floats must error"
        );
        assert!(
            lua.load("Decals.Spawn(1, 2, 3, 'up', 1, 0)")
                .exec()
                .is_err(),
            "a non-numeric required arg must error"
        );
        assert_eq!(scene.borrow().decals.len(), 0, "no partial stamp on error");
        Ok(())
    })
    .unwrap();
}
