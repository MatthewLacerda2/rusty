//! `Physics.*` bindings vs `authoring::rigidbody`.

use std::cell::RefCell;

use glam::Vec3;
use mlua::Lua;
use rusty::components::{CollisionDetection, RigidBodyComponent};
use rusty::scene::authoring::rigidbody as rb_ops;
use rusty::scene::Scene;

/// Attach a default rigidbody to a fresh entity; returns its id.
fn entity_with_rb(scene: &mut Scene, name: &str) -> u32 {
    let id = scene.add_entity(name.to_string());
    scene.world.set_rigidbody(
        id,
        Some(RigidBodyComponent {
            active: true,
            is_kinematic: false,
            mass: 1.0,
            velocity: Vec3::ZERO,
            angular_velocity: Vec3::ZERO,
            use_gravity: true,
            collision_detection: CollisionDetection::Discrete,
        }),
    );
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
