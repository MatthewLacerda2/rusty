//! The navigation areas through Lua (#460): the area table on `Navigation`, the
//! agent's mask on `NavMeshAgent`, and the optional mask on the path queries.

use std::cell::RefCell;

use glam::Vec3;
use mlua::{Lua, Table};

use super::register;
use crate::navigation::test_support::{add_floor, add_modifier, bake_pinned};
use crate::navigation::NavigationGraph;
use crate::scene::{NavMeshAgentComponent, Scene};

/// A floor with a wall of "Fire" (area 2) across the pinned grid at x 5, and an agent.
fn fire_world() -> (RefCell<Scene>, RefCell<NavigationGraph>, u32) {
    let mut scene = Scene::new();
    scene.nav_settings.agent_radius = 0.0;
    scene.nav_settings.define_area("Fire", 1.0);
    add_floor(&mut scene, -5.0, 15.0, -5.0, 15.0);
    let wall = Vec3::new(1.0, 2.0, 30.0);
    add_modifier(&mut scene, Vec3::new(5.0, 0.0, 5.0), wall, 2);
    let nav = RefCell::new(bake_pinned(&mut scene));
    let id = scene.add_entity("agent".to_string());
    let agent = NavMeshAgentComponent::default();
    scene.world.set_nav_agent(id, Some(agent));
    (RefCell::new(scene), nav, id)
}

#[test]
fn the_area_table_is_edited_by_name_without_a_rebake() {
    let (scene, nav, _) = fire_world();
    let generation = nav.borrow().bake_generation;
    let lua = Lua::new();
    lua.scope(|scope| {
        register(&lua, scope, &scene, &nav).unwrap();
        let ids: (i32, i32, u8) = lua
            .load(r#"return Navigation.GetAreaFromName("Fire"), Navigation.GetAreaFromName("Ice"), Navigation.DefineArea("Ice", 3)"#)
            .eval()?;
        assert_eq!(ids, (2, -1, 3));
        lua.load(r#"Navigation.SetAreaCost("Fire", 0.5)"#).exec()?;
        let cost: f32 = lua.load(r#"return Navigation.GetAreaCost("Fire")"#).eval()?;
        assert_eq!(cost, 1.0, "costs clamp to 1");
        assert!(lua.load(r#"Navigation.SetAreaCost("Smoke", 2)"#).exec().is_err());
        lua.load(r#"Navigation.SetAreaCost("Fire", 8)"#).exec()?;
        let areas: Table = lua.load("return Navigation.GetAreas()").eval()?;
        assert_eq!(areas.raw_len(), 4);
        Ok(())
    })
    .unwrap();
    assert_eq!(
        nav.borrow().area_cost(2),
        8.0,
        "the live graph has the cost"
    );
    assert_ne!(nav.borrow().bake_generation, generation);
    assert_eq!(
        scene.borrow().nav_settings.areas[2].cost,
        8.0,
        "and the scene"
    );
}

#[test]
fn the_agent_mask_and_the_masked_queries_keep_out_of_an_area() {
    let (scene, nav, id) = fire_world();
    let lua = Lua::new();
    lua.scope(|scope| {
        register(&lua, scope, &scene, &nav).unwrap();
        lua.globals().set("id", id)?;
        let mask: i64 = lua.load("return NavMeshAgent.GetAreaMask(id)").eval()?;
        assert_eq!(mask, i64::from(u32::MAX), "every area by default");
        lua.load("NavMeshAgent.SetAreaMask(id, -1 ~ 4)").exec()?;
        let path: Table = lua
            .load("return Navigation.CalculatePath(2, 0, 2, 8, 0, 2, NavMeshAgent.GetAreaMask(id))")
            .eval()?;
        assert_eq!(path.get::<_, String>("status")?, "partial", "fire blocks");
        let open: Table = lua
            .load("return Navigation.CalculatePath(2, 0, 2, 8, 0, 2)")
            .eval()?;
        assert_eq!(open.get::<_, String>("status")?, "complete");
        let (hit,): (bool,) = lua
            .load("return Navigation.Raycast(2, 0, 2, 8, 0, 2, -1 ~ 4)")
            .eval()?;
        assert!(hit, "the masked walk stops at the fire");
        let (found,): (bool,) = lua
            .load("return Navigation.SamplePosition(5, 0, 5, 0.4, -1 ~ 4)")
            .eval()?;
        assert!(!found);
        Ok(())
    })
    .unwrap();
    assert_eq!(scene.borrow().world.nav_agent(id).unwrap().area_mask, !4);
}
