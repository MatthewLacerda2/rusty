//! `Physics.*` bindings vs `authoring::physics::rigidbody`.

use std::cell::RefCell;

use glam::Vec3;
use mlua::Lua;
use rusty::components::CollisionDetection;
use rusty::scene::authoring::physics::rigidbody as rb_ops;
use rusty::scene::Scene;

/// Attach a default rigidbody to a fresh entity; returns its id.
fn entity_with_rb(scene: &mut Scene, name: &str) -> u32 {
    let id = scene.add_entity(name.to_string());
    scene
        .world
        .set_rigidbody(id, Some(rusty::scene::authoring::default_rigidbody()));
    id
}

#[test]
fn physics_api_and_shared_op_converge() -> Result<(), Box<dyn std::error::Error>> {
    let scene = RefCell::new(Scene::new());
    let via_lua = entity_with_rb(&mut scene.borrow_mut(), "ViaLua");
    let via_op = entity_with_rb(&mut scene.borrow_mut(), "ViaOp");

    let lua = Lua::new();
    lua.scope(|s| {
        rusty::api::physics::register(&lua, s, &scene).unwrap();
        lua.load(format!(
            r#"
            Physics.SetVelocity({via_lua}, 1.0, 2.0, 3.0)
            Physics.SetKinematic({via_lua}, true)
            Physics.SetAngularVelocity({via_lua}, 0.0, 4.0, 0.0)
            Physics.SetCollisionDetection({via_lua}, "Continuous")
        "#
        ))
        .exec()
        .unwrap();
        Ok(())
    })?;

    {
        let mut sc = scene.borrow_mut();
        let mut e = sc.world.rigidbody_mut(via_op).unwrap();
        rb_ops::set_velocity(&mut e, Vec3::new(1.0, 2.0, 3.0));
        rb_ops::set_kinematic(&mut e, true);
        rb_ops::set_angular_velocity(&mut e, Vec3::new(0.0, 4.0, 0.0));
        rb_ops::set_collision_detection(&mut e, CollisionDetection::Continuous);
    }

    let sc = scene.borrow();
    let a = sc.world.rigidbody(via_lua).unwrap().clone();
    let b = sc.world.rigidbody(via_op).unwrap().clone();
    assert_eq!(a.velocity, b.velocity);
    assert_eq!(a.is_kinematic, b.is_kinematic);
    assert_eq!(a.angular_velocity, b.angular_velocity);
    assert_eq!(a.collision_detection, b.collision_detection);
    Ok(())
}

/// `SetMass` clamps through the op the card uses; `SetUseGravity` / `SetTrigger`
/// write through theirs and the getters read them back (#1365).
#[test]
fn mass_gravity_and_trigger_reach_their_ops() -> Result<(), Box<dyn std::error::Error>> {
    let scene = RefCell::new(Scene::new());
    let id = entity_with_rb(&mut scene.borrow_mut(), "Body");
    let collider = rusty::scene::authoring::default_collider();
    scene.borrow_mut().world.set_collider(id, Some(collider));

    let lua = Lua::new();
    let read: (f32, f32, bool, bool, f32) = lua.scope(|s| {
        rusty::api::physics::register(&lua, s, &scene).unwrap();
        lua.load(format!(
            r#"
            local before = Physics.GetMass({id})
            Physics.SetMass({id}, 5000)
            Physics.SetUseGravity({id}, false)
            Physics.SetTrigger({id}, true)
            Physics.SetMass(9999, 3)  -- no Rigidbody: a no-op
            return before, Physics.GetMass({id}), Physics.GetUseGravity({id}),
                Physics.IsTrigger({id}), Physics.GetMass(9999)
        "#
        ))
        .eval()
    })?;

    let mut expected = rusty::scene::authoring::default_rigidbody();
    rb_ops::set_mass(&mut expected, 5000.0);
    assert_eq!(read, (1.0, expected.mass, false, true, 0.0));
    assert_eq!(expected.mass, 1000.0, "the card's clamp");
    Ok(())
}
