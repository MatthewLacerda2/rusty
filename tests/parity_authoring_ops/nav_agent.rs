//! `NavMeshAgent.*` bindings vs `authoring::nav_agent`.

use std::cell::RefCell;

use glam::Vec3;
use mlua::Lua;
use rusty::components::NavMeshAgentComponent;
use rusty::navigation::NavigationGraph;
use rusty::scene::authoring::nav_agent as nav_ops;
use rusty::scene::Scene;

/// Attach a default nav-agent to a fresh entity; returns its id.
fn entity_with_agent(scene: &mut Scene, name: &str) -> u32 {
    let id = scene.add_entity(name.to_string());
    scene
        .world
        .set_nav_agent(id, Some(NavMeshAgentComponent::default()));
    id
}

#[test]
fn nav_agent_api_and_shared_op_converge() -> Result<(), Box<dyn std::error::Error>> {
    let scene = RefCell::new(Scene::new());
    let via_lua = entity_with_agent(&mut scene.borrow_mut(), "ViaLua");
    let via_op = entity_with_agent(&mut scene.borrow_mut(), "ViaOp");
    let nav = RefCell::new(NavigationGraph::new(0.0, 20.0, 0.0, 20.0, 1.0));

    let lua = Lua::new();
    lua.scope(|s| {
        rusty::api::nav::register(&lua, s, &scene, &nav).unwrap();
        lua.load(format!(
            r#"
            NavMeshAgent.SetActive({via_lua}, true)
            NavMeshAgent.SetSpeed({via_lua}, 3.5)
            NavMeshAgent.SetAcceleration({via_lua}, 8.0)
            NavMeshAgent.SetStoppingDistance({via_lua}, 0.5)
            NavMeshAgent.SetRadius({via_lua}, 0.4)
            NavMeshAgent.SetTarget({via_lua}, 1.0, 0.0, 2.0)
            NavMeshAgent.SetAvoidancePriority({via_lua}, 17)
            NavMeshAgent.SetAvoidanceEnabled({via_lua}, false)
        "#
        ))
        .exec()
        .unwrap();
        Ok(())
    })?;

    {
        let mut sc = scene.borrow_mut();
        let mut e = sc.world.nav_agent_mut(via_op).unwrap();
        nav_ops::set_active(&mut e, true);
        nav_ops::set_speed(&mut e, 3.5);
        nav_ops::set_acceleration(&mut e, 8.0);
        nav_ops::set_stopping_distance(&mut e, 0.5);
        nav_ops::set_radius(&mut e, 0.4);
        nav_ops::set_target(&mut e, Vec3::new(1.0, 0.0, 2.0));
        nav_ops::set_avoidance_priority(&mut e, 17.0);
        nav_ops::set_avoidance_enabled(&mut e, false);
    }

    let sc = scene.borrow();
    let a = sc.world.nav_agent(via_lua).unwrap().clone();
    let b = sc.world.nav_agent(via_op).unwrap().clone();
    assert_eq!(a.active, b.active);
    assert_eq!(a.speed, b.speed);
    assert_eq!(a.acceleration, b.acceleration);
    assert_eq!(a.stopping_distance, b.stopping_distance);
    assert_eq!(a.radius, b.radius);
    assert_eq!(a.target, b.target);
    assert_eq!((a.avoidance_priority, a.avoidance_enabled), (17, false));
    assert_eq!(a.avoidance_priority, b.avoidance_priority);
    assert_eq!(a.avoidance_enabled, b.avoidance_enabled);
    Ok(())
}

#[test]
fn base_offset_api_and_shared_op_converge() -> Result<(), Box<dyn std::error::Error>> {
    let scene = RefCell::new(Scene::new());
    let via_lua = entity_with_agent(&mut scene.borrow_mut(), "ViaLua");
    let via_op = entity_with_agent(&mut scene.borrow_mut(), "ViaOp");
    let nav = RefCell::new(NavigationGraph::new(0.0, 20.0, 0.0, 20.0, 1.0));
    let lua = Lua::new();
    let read = lua.scope(|s| {
        rusty::api::nav::register(&lua, s, &scene, &nav).unwrap();
        let src = format!(
            "NavMeshAgent.SetBaseOffset({via_lua}, 1.0) return NavMeshAgent.GetBaseOffset({via_lua})"
        );
        lua.load(src).eval::<f32>()
    })?;
    nav_ops::set_base_offset(
        &mut scene.borrow_mut().world.nav_agent_mut(via_op).unwrap(),
        1.0,
    );
    let sc = scene.borrow();
    let (a, b) = (
        sc.world.nav_agent(via_lua).unwrap(),
        sc.world.nav_agent(via_op).unwrap(),
    );
    assert_eq!((read, a.base_offset, b.base_offset), (1.0, 1.0, 1.0));
    Ok(())
}

#[test]
fn turning_api_and_shared_op_converge() -> Result<(), Box<dyn std::error::Error>> {
    let scene = RefCell::new(Scene::new());
    let via_lua = entity_with_agent(&mut scene.borrow_mut(), "ViaLua");
    let via_op = entity_with_agent(&mut scene.borrow_mut(), "ViaOp");
    let nav = RefCell::new(NavigationGraph::new(0.0, 20.0, 0.0, 20.0, 1.0));
    let lua = Lua::new();
    let read: (bool, f32, f32) = lua.scope(|s| {
        rusty::api::nav::register(&lua, s, &scene, &nav).unwrap();
        lua.load(format!(
            "NavMeshAgent.SetUpdateRotation({via_lua}, false)
             NavMeshAgent.SetAngularSpeed({via_lua}, 90)
             NavMeshAgent.SetAngularAcceleration({via_lua}, 360)
             return NavMeshAgent.GetUpdateRotation({via_lua}),
                    NavMeshAgent.GetAngularSpeed({via_lua}),
                    NavMeshAgent.GetAngularAcceleration({via_lua})"
        ))
        .eval()
    })?;
    {
        let mut sc = scene.borrow_mut();
        let mut e = sc.world.nav_agent_mut(via_op).unwrap();
        nav_ops::set_update_rotation(&mut e, false);
        nav_ops::set_angular_speed(&mut e, 90.0);
        nav_ops::set_angular_acceleration(&mut e, 360.0);
    }
    let sc = scene.borrow();
    let turning = |id| {
        let a = sc.world.nav_agent(id).unwrap();
        (a.update_rotation, a.angular_speed, a.angular_acceleration)
    };
    assert_eq!(read, (false, 90.0, 360.0));
    assert_eq!(turning(via_lua), turning(via_op));
    Ok(())
}
