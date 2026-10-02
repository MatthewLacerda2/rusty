//! The `NavMeshObstacle` namespace (#456), through Lua: setters route through the
//! shared ops (clamped), getters read back, and an entity without an obstacle
//! reads neutral defaults.

use std::cell::RefCell;

use mlua::Lua;

use super::register;
use crate::components::NavMeshObstacleComponent;
use crate::scene::Scene;

fn scene_with_obstacle() -> (RefCell<Scene>, u32, u32) {
    let mut scene = Scene::new();
    let id = scene.add_entity("door".to_string());
    scene
        .world
        .set_nav_obstacle(id, Some(NavMeshObstacleComponent::default()));
    let bare = scene.add_entity("bare".to_string());
    (RefCell::new(scene), id, bare)
}

#[test]
fn setters_write_through_the_shared_ops_and_getters_read_back() {
    let (scene, id, _) = scene_with_obstacle();
    let lua = Lua::new();
    lua.scope(|scope| {
        register(&lua, scope, &scene).unwrap();
        lua.globals().set("id", id)?;
        lua.load(
            "NavMeshObstacle.SetShape(id, 'capsule')
             NavMeshObstacle.SetRadius(id, 0.75)
             NavMeshObstacle.SetHeight(id, -1)
             NavMeshObstacle.SetSize(id, 2, 3, 4)
             NavMeshObstacle.SetCenter(id, 0, 1, 0)
             NavMeshObstacle.SetCarving(id, true)
             NavMeshObstacle.SetCarveOnlyStationary(id, false)
             NavMeshObstacle.SetMoveThreshold(id, 0.25)
             NavMeshObstacle.SetTimeToStationary(id, 1.5)",
        )
        .exec()?;
        let shape: String = lua.load("return NavMeshObstacle.GetShape(id)").eval()?;
        assert_eq!(shape, "Capsule");
        let (r, h): (f32, f32) = lua
            .load("return NavMeshObstacle.GetRadius(id), NavMeshObstacle.GetHeight(id)")
            .eval()?;
        assert_eq!((r, h), (0.75, 1e-3), "a negative height clamps");
        let size: (f32, f32, f32) = lua.load("return NavMeshObstacle.GetSize(id)").eval()?;
        assert_eq!(size, (2.0, 3.0, 4.0));
        let carving: (bool, bool, bool) = lua
            .load(
                "return NavMeshObstacle.GetCarving(id), NavMeshObstacle.GetCarveOnlyStationary(id),
                 NavMeshObstacle.IsCarving(id)",
            )
            .eval()?;
        assert_eq!(carving, (true, false, true));
        let bad = lua.load("NavMeshObstacle.SetShape(id, 'cone')").exec();
        assert!(bad.is_err(), "an unknown shape is an error");
        Ok(())
    })
    .unwrap();
}

#[test]
fn an_entity_without_an_obstacle_reads_defaults() {
    let (scene, _, bare) = scene_with_obstacle();
    let lua = Lua::new();
    lua.scope(|scope| {
        register(&lua, scope, &scene).unwrap();
        lua.globals().set("bare", bare)?;
        lua.load("NavMeshObstacle.SetCarving(bare, true)").exec()?;
        let (carving, active): (bool, bool) = lua
            .load("return NavMeshObstacle.GetCarving(bare), NavMeshObstacle.GetActive(bare)")
            .eval()?;
        assert_eq!((carving, active), (false, false));
        let shape: Option<String> = lua.load("return NavMeshObstacle.GetShape(bare)").eval()?;
        assert_eq!(shape, None);
        Ok(())
    })
    .unwrap();
}
