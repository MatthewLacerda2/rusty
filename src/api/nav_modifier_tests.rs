//! The `NavMeshModifierVolume` namespace (#460), through Lua: setters route through
//! the shared ops, getters read back, and an entity without a volume reads neutral
//! defaults and is never given one.

use std::cell::RefCell;

use mlua::Lua;

use super::register;
use crate::components::NavMeshModifierVolumeComponent;
use crate::scene::Scene;

#[test]
fn setters_write_through_the_shared_ops_and_getters_read_back() {
    let mut scene = Scene::new();
    let id = scene.add_entity("mud".to_string());
    scene
        .world
        .set_nav_modifier(id, Some(NavMeshModifierVolumeComponent::default()));
    let bare = scene.add_entity("bare".to_string());
    let scene = RefCell::new(scene);
    let lua = Lua::new();
    lua.scope(|scope| {
        register(&lua, scope, &scene).unwrap();
        lua.globals().set("id", id)?;
        lua.globals().set("bare", bare)?;
        lua.load(
            "NavMeshModifierVolume.SetCenter(id, 1, 0, 2)
             NavMeshModifierVolume.SetSize(id, 6, -1, 2)
             NavMeshModifierVolume.SetArea(id, 3)
             NavMeshModifierVolume.SetArea(id, 99)
             NavMeshModifierVolume.SetActive(id, false)
             NavMeshModifierVolume.SetArea(bare, 3)",
        )
        .exec()?;
        let center: (f32, f32, f32) = lua
            .load("return NavMeshModifierVolume.GetCenter(id)")
            .eval()?;
        assert_eq!(center, (1.0, 0.0, 2.0));
        let size: (f32, f32, f32) = lua
            .load("return NavMeshModifierVolume.GetSize(id)")
            .eval()?;
        assert_eq!(size, (6.0, 1e-3, 2.0), "sizes stay positive");
        let opts: (u8, bool) = lua
            .load("return NavMeshModifierVolume.GetArea(id), NavMeshModifierVolume.GetActive(id)")
            .eval()?;
        assert_eq!(opts, (3, false), "an out-of-range area is ignored");
        let none: (u8, bool) = lua
            .load(
                "return NavMeshModifierVolume.GetArea(bare), NavMeshModifierVolume.GetActive(bare)",
            )
            .eval()?;
        assert_eq!(none, (0, false));
        Ok(())
    })
    .unwrap();
    assert!(
        scene.borrow().world.nav_modifier(bare).is_none(),
        "setters never attach"
    );
}
