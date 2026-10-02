//! The `Navigation` path queries and `NavMeshAgent` path state (#458), through Lua:
//! each binding reaches the navmesh query it names and returns the documented shape.

use std::cell::RefCell;

use glam::Vec3;
use mlua::{Lua, Table};

use super::register;
use crate::navigation::test_support::{add_floor, bake_pinned};
use crate::navigation::NavigationGraph;
use crate::scene::{NavMeshAgentComponent, Scene};

/// An open floor over the pinned grid, with one agent at (2, 0, 2) headed to (8, 0, 2).
fn world() -> (RefCell<Scene>, RefCell<NavigationGraph>, u32) {
    let mut scene = Scene::new();
    add_floor(&mut scene, -5.0, 15.0, -5.0, 15.0);
    let nav = bake_pinned(&mut scene);
    let id = scene.add_entity("agent".to_string());
    scene.world.transform_mut(id).expect("transform").position = Vec3::new(2.0, 0.0, 2.0);
    let agent = NavMeshAgentComponent {
        active: true,
        target: Vec3::new(8.0, 0.0, 2.0),
        speed: 4.0,
        acceleration: 20.0,
        ..Default::default()
    };
    scene.world.set_nav_agent(id, Some(agent));
    (RefCell::new(scene), RefCell::new(nav), id)
}

#[test]
fn navigation_queries_return_their_documented_shapes() {
    let (scene, nav, _) = world();
    let lua = Lua::new();
    lua.scope(|scope| {
        register(&lua, scope, &scene, &nav).unwrap();
        let path: Table = lua
            .load("return Navigation.CalculatePath(2, 0, 2, 8, 0, 7)")
            .eval()?;
        assert_eq!(path.get::<_, String>("status")?, "complete");
        let corners: Table = path.get("corners")?;
        assert_eq!(corners.raw_len(), 2, "one straight run");
        let end: Table = corners.get(2)?;
        assert_eq!(
            (end.get::<_, f32>("x")?, end.get::<_, f32>("z")?),
            (8.0, 7.0)
        );
        let length: f32 = path.get("length")?;
        assert!((length - 61f32.sqrt()).abs() < 1e-4);
        let helper: f32 = lua
            .load("return Navigation.GetPathLength(Navigation.CalculatePath(2, 0, 2, 8, 0, 7))")
            .eval()?;
        assert_eq!(helper, length);

        let (found, y): (bool, f32) = lua
            .load("local f, x, y, z = Navigation.SamplePosition(3, 1.2, 3, 2); return f, y")
            .eval()?;
        assert!(found && y == 0.0, "snapped down onto the floor");
        let (hit, x): (bool, f32) = lua
            .load("local h, x = Navigation.Raycast(2, 0, 2, 8, 0, 2); return h, x")
            .eval()?;
        assert!(!hit && x == 8.0, "a clear run ends on the target");
        Ok(())
    })
    .unwrap();
}

#[test]
fn agent_path_state_reads_warp_and_reset() {
    let (scene, nav, id) = world();
    let lua = Lua::new();
    lua.globals().set("id", id).unwrap();
    lua.scope(|scope| {
        register(&lua, scope, &scene, &nav).unwrap();
        let before: (bool, String, f32) = lua
            .load("return NavMeshAgent.HasPath(id), NavMeshAgent.GetPathStatus(id), NavMeshAgent.RemainingDistance(id)")
            .eval()?;
        assert_eq!((before.0, before.1.as_str()), (false, "invalid"));
        assert!(before.2.is_infinite(), "unknown distance is infinity");

        nav.borrow().tick_nav_agents(&mut scene.borrow_mut(), 1.0 / 60.0);
        let (has, status, left): (bool, String, f32) = lua
            .load("return NavMeshAgent.HasPath(id), NavMeshAgent.GetPathStatus(id), NavMeshAgent.RemainingDistance(id)")
            .eval()?;
        assert!(has && status == "complete");
        let x = scene.borrow().world.transform(id).expect("t").position.x;
        assert!((left - (8.0 - x)).abs() < 1e-4);
        let corners: usize = lua.load("return #NavMeshAgent.GetPath(id).corners").eval()?;
        assert_eq!(corners, 2, "where it stands, then the target");

        let warped: bool = lua.load("return NavMeshAgent.Warp(id, 5, 0.5, 9)").eval()?;
        assert!(warped);
        let at = scene.borrow().world.transform(id).expect("t").position;
        assert_eq!(at, Vec3::new(5.0, 0.0, 9.0), "landed on the floor");
        let off: bool = lua.load("return NavMeshAgent.Warp(id, 5, 30, 9)").eval()?;
        assert!(!off, "no navmesh within reach");

        lua.load("NavMeshAgent.ResetPath(id)").exec()?;
        let target: (f32, f32, f32) = lua.load("return NavMeshAgent.GetTarget(id)").eval()?;
        assert_eq!(target, (5.0, 0.0, 9.0), "the destination is where it stands");
        let has: bool = lua.load("return NavMeshAgent.HasPath(id)").eval()?;
        assert!(!has);
        Ok(())
    })
    .unwrap();
}
