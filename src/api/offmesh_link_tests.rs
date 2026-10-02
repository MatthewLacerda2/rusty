//! The `OffMeshLink` namespace (#462), through Lua: setters route through the
//! shared ops, getters read back, `IsConnected` reads the baked navmesh, and an
//! entity without a link reads neutral defaults.

use std::cell::RefCell;

use glam::Vec3;
use mlua::Lua;

use super::register;
use crate::components::OffMeshLinkComponent;
use crate::navigation::{NavBounds, NavigationGraph};
use crate::scene::{ColliderComponent, ColliderShape, Scene};

/// A 10 × 10 floor, a link on it, and a bare entity.
fn scene_with_link() -> (Scene, u32, u32) {
    let mut scene = Scene::new();
    scene.nav_settings.bounds = Some(NavBounds::new(0.0, 10.0, 0.0, 10.0));
    let floor = scene.add_entity("floor".to_string());
    scene.world.set_static(floor, true);
    scene.world.transform_mut(floor).unwrap().position = Vec3::new(5.0, -0.05, 5.0);
    let mut col = ColliderComponent::from_mesh_bounds(false, Vec3::ZERO, Vec3::ZERO);
    col.shape = ColliderShape::Box {
        size: Vec3::new(12.0, 0.1, 12.0),
    };
    scene.world.set_collider(floor, Some(col));
    scene.update_entity_collider(floor);
    let id = scene.add_entity("ladder".to_string());
    scene.world.transform_mut(id).unwrap().position = Vec3::new(3.0, 0.0, 3.0);
    scene
        .world
        .set_offmesh_link(id, Some(OffMeshLinkComponent::default()));
    let bare = scene.add_entity("bare".to_string());
    (scene, id, bare)
}

#[test]
fn setters_write_through_the_shared_ops_and_getters_read_back() {
    let (scene, id, bare) = scene_with_link();
    let nav = RefCell::new(NavigationGraph::from_scene(&scene));
    nav.borrow_mut().bake(&scene);
    let scene = RefCell::new(scene);
    let lua = Lua::new();
    lua.scope(|scope| {
        register(&lua, scope, &scene, &nav).unwrap();
        lua.globals().set("id", id)?;
        lua.globals().set("bare", bare)?;
        let connected: bool = lua.load("return OffMeshLink.IsConnected(id)").eval()?;
        assert!(connected, "both ends stand on the floor");
        lua.load(
            "OffMeshLink.SetStart(id, 0, 0, 1)
             OffMeshLink.SetEnd(id, 0, 2, 4)
             OffMeshLink.SetBidirectional(id, false)
             OffMeshLink.SetCost(id, -5)
             OffMeshLink.SetActive(id, false)
             OffMeshLink.SetCost(bare, 3)",
        )
        .exec()?;
        let end: (f32, f32, f32) = lua.load("return OffMeshLink.GetEnd(id)").eval()?;
        assert_eq!(end, (0.0, 2.0, 4.0));
        let start: (f32, f32, f32) = lua.load("return OffMeshLink.GetStart(id)").eval()?;
        assert_eq!(start, (0.0, 0.0, 1.0));
        let flags: (bool, bool, f32) = lua
            .load("return OffMeshLink.GetActive(id), OffMeshLink.GetBidirectional(id), OffMeshLink.GetCost(id)")
            .eval()?;
        assert_eq!(flags, (false, false, -1.0), "a negative cost means the length");
        let none: (bool, f32, bool) = lua
            .load("return OffMeshLink.GetActive(bare), OffMeshLink.GetCost(bare), OffMeshLink.IsConnected(bare)")
            .eval()?;
        assert_eq!(none, (false, -1.0, false));
        Ok(())
    })
    .unwrap();
    assert!(
        scene.borrow().world.offmesh_link(bare).is_none(),
        "setters never attach"
    );
}
