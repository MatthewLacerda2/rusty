//! src/scripting/tests_collider.rs — the collider shape + physics material
//! bindings (#447): `Physics.Get/SetColliderShape`, `Physics.Get/SetPhysicsMaterial`.

use std::cell::RefCell;
use std::rc::Rc;

use super::console::ConsoleLogs;
use super::manager::ScriptManager;
use crate::components::{CapsuleAxis, ColliderShape, CombineMode, PhysicsMaterial};
use crate::core::input::InputState;
use crate::navigation::NavigationGraph;
use crate::physics::PhysicsWorld;
use crate::scene::authoring::defaults::default_collider;
use crate::scene::{Camera, Scene};
use crate::time::Time;
use glam::Vec3;

/// A live `ScriptManager` over a scene with one Box-collider entity at y = 3.
fn manager_with_collider() -> (ScriptManager, Rc<RefCell<Scene>>, u32) {
    let mut raw = Scene::new();
    let id = raw.add_entity("Body".to_string());
    raw.world.transform_mut(id).unwrap().position = Vec3::new(0.0, 3.0, 0.0);
    raw.world.set_collider(id, Some(default_collider()));
    let scene = Rc::new(RefCell::new(raw));
    let input = Rc::new(RefCell::new(InputState::new()));
    let nav = Rc::new(RefCell::new(NavigationGraph::new(
        -10.0, 10.0, -10.0, 10.0, 1.0,
    )));
    let console = Rc::new(RefCell::new(ConsoleLogs::new()));
    let camera = Rc::new(RefCell::new(Camera::new(Vec3::ZERO, 0.0, 0.0)));
    let time = Rc::new(RefCell::new(Time::new()));
    let mut m = ScriptManager::new(Rc::clone(&scene), input, nav, console, camera, time);
    let physics = Rc::new(RefCell::new(None::<PhysicsWorld>));
    m.init_runtime(&physics).expect("runtime inits");
    (m, scene, id)
}

#[test]
fn capsule_shape_round_trips_and_refreshes_the_bounds() {
    let (m, scene, id) = manager_with_collider();
    m.eval(&format!(
        "Physics.SetColliderShape({id}, {{kind='Capsule', radius=0.5, height=2, axis='Z'}})"
    ))
    .unwrap();
    let shape = scene.borrow().world.collider(id).unwrap().shape.clone();
    let want = ColliderShape::Capsule {
        radius: 0.5,
        height: 2.0,
        axis: CapsuleAxis::Z,
    };
    assert_eq!(shape, want);
    let (min, max) = {
        let s = scene.borrow();
        let c = s.world.collider(id).unwrap();
        (c.aabb_min, c.aabb_max)
    };
    assert_eq!(
        (min, max),
        (Vec3::new(-0.5, 2.5, -1.0), Vec3::new(0.5, 3.5, 1.0))
    );
    let read = m
        .eval(&format!(
            "local s = Physics.GetColliderShape({id}); \
             return s.kind .. ' ' .. s.radius .. ' ' .. s.height .. ' ' .. s.axis"
        ))
        .unwrap();
    assert_eq!(read.trim(), "Capsule 0.5 2.0 Z");
    // An omitted axis is Unity's default, Y.
    m.eval(&format!(
        "Physics.SetColliderShape({id}, {{kind='Capsule', radius=1, height=3}})"
    ))
    .unwrap();
    let axis = m
        .eval(&format!("return Physics.GetColliderShape({id}).axis"))
        .unwrap();
    assert_eq!(axis.trim(), "Y");
}

#[test]
fn box_shape_round_trips_and_bad_shapes_are_errors() {
    let (m, scene, id) = manager_with_collider();
    m.eval(&format!(
        "Physics.SetColliderShape({id}, {{kind='Box', x=1, y=2, z=3}})"
    ))
    .unwrap();
    let size = Vec3::new(1.0, 2.0, 3.0);
    let got = scene.borrow().world.collider(id).unwrap().shape.clone();
    assert_eq!(got, ColliderShape::Box { size });
    for bad in [
        "{kind='Capsule', radius=0, height=2}",
        "{kind='Capsule', radius=0.5, height=2, axis='W'}",
        "{kind='Sphere'}",
        "{kind='Mesh'}",
    ] {
        let lua = format!("Physics.SetColliderShape({id}, {bad})");
        assert!(m.eval(&lua).is_err(), "{bad} must be rejected");
    }
    let still = scene.borrow().world.collider(id).unwrap().shape.clone();
    assert_eq!(
        still,
        ColliderShape::Box { size },
        "a rejected shape writes nothing"
    );
    let none = m
        .eval("return Physics.GetColliderShape(9999) == nil")
        .unwrap();
    assert_eq!(none.trim(), "true");
}

#[test]
fn physics_material_round_trips_clamps_and_rejects_unknown_modes() {
    let (m, scene, id) = manager_with_collider();
    let read = |m: &ScriptManager| {
        let lua = format!(
            "local f, b, fc, bc = Physics.GetPhysicsMaterial({id}); \
             return f .. ' ' .. b .. ' ' .. fc .. ' ' .. bc"
        );
        m.eval(&lua).unwrap().trim().to_string()
    };
    assert_eq!(read(&m), "0.5 0.0 Average Average");
    m.eval(&format!(
        "Physics.SetPhysicsMaterial({id}, 0.25, 0.75, 'Minimum', 'Maximum')"
    ))
    .unwrap();
    assert_eq!(read(&m), "0.25 0.75 Minimum Maximum");
    let stored = scene.borrow().world.collider(id).unwrap().material;
    assert_eq!(
        stored,
        PhysicsMaterial {
            friction_combine: CombineMode::Minimum,
            bounce_combine: CombineMode::Maximum,
            ..PhysicsMaterial::new(0.25, 0.75)
        }
    );
    // Omitted modes fall back to Average; out-of-range values clamp.
    m.eval(&format!("Physics.SetPhysicsMaterial({id}, -1, 3)"))
        .unwrap();
    assert_eq!(read(&m), "0.0 1.0 Average Average");
    let bad = format!("Physics.SetPhysicsMaterial({id}, 0.5, 0.5, 'Max')");
    assert!(
        m.eval(&bad).is_err(),
        "an unknown combine mode is a script error"
    );
    let nan = format!("Physics.SetPhysicsMaterial({id}, 0/0, 0.5)");
    assert!(
        m.eval(&nan).is_err(),
        "a non-finite coefficient is a script error"
    );
}
