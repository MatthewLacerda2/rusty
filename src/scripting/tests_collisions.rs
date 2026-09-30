//! Tests for the collision callbacks (#448): `OnCollisionEnter` / `OnCollisionStay`
//! get `(id, other, contact)` as the receiver sees it, `OnCollisionExit` gets
//! `(id, other)`, and they dispatch after the trigger callbacks of the same tick.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use glam::Vec3;

use super::tests_lifecycle::{manager_with_entity, write_script};
use crate::physics::{CollisionEvents, CollisionPair, Contact, PhysicsEvents, TriggerEvents};

/// `id` resting on entity 9: `id` sees the floor push up and close at 3 m/s.
fn landing(id: u32) -> CollisionPair {
    let contact = Contact {
        point: Vec3::new(1.0, 0.0, 2.0),
        normal: Vec3::Y,
        relative_velocity: Vec3::new(0.0, 3.0, 0.0),
        impulse: 4.5,
        other_body: 9,
    };
    CollisionPair {
        a: id,
        b: 9,
        contact,
        body_a: id,
    }
}

#[test]
fn collision_callbacks_get_the_receivers_contact() {
    let (mut m, id) = manager_with_entity();
    m.init_runtime(&Rc::new(RefCell::new(None))).unwrap();
    let path = write_script(
        "hits",
        "_G.__log = ''\nreturn {\n\
         OnTrigger = function(id, other) __log = __log .. 'T' end,\n\
         OnCollisionEnter = function(id, other, c) __log = __log .. 'E' .. other\n\
           .. ':' .. c.normal.y .. ',' .. c.point.z .. ',' .. c.relativeVelocity.y\n\
           .. ',' .. c.impulse .. ',' .. c.otherBody end,\n\
         OnCollisionStay = function(id, other, c) __log = __log .. 'S' end,\n\
         OnCollisionExit = function(id, other) __log = __log .. 'X' .. other end,\n\
         }",
    );
    m.load_entity_script(id, 0, &path, &BTreeMap::new())
        .unwrap();
    m.init_scripts(); // physics callbacks only reach awoken instances (#322)

    // Triggers of the tick first, then enter before stay.
    m.dispatch_physics_events(PhysicsEvents {
        triggers: TriggerEvents {
            stayed: vec![(id, 5)],
            ..Default::default()
        },
        collisions: CollisionEvents {
            entered: vec![landing(id)],
            stayed: vec![landing(id)],
            ..Default::default()
        },
    });
    assert_eq!(m.eval("__log").unwrap(), "TE9:1.0,2.0,3.0,4.5,9S");

    m.dispatch_physics_events(PhysicsEvents {
        collisions: CollisionEvents {
            exited: vec![(id, 9)],
            ..Default::default()
        },
        ..Default::default()
    });
    assert_eq!(m.eval("__log").unwrap(), "TE9:1.0,2.0,3.0,4.5,9SX9");
}

#[test]
fn the_other_side_sees_the_contact_flipped() {
    let (mut m, id) = manager_with_entity();
    m.init_runtime(&Rc::new(RefCell::new(None))).unwrap();
    let path = write_script(
        "flip",
        "_G.__n = ''\nreturn { OnCollisionEnter = function(id, other, c)\n\
         __n = c.normal.y .. ',' .. c.relativeVelocity.y .. ',' .. c.otherBody end }",
    );
    m.load_entity_script(id, 0, &path, &BTreeMap::new())
        .unwrap();
    m.init_scripts();
    // Keyed from another entity's side, with `id` as its `b`.
    let mut pair = landing(id + 100);
    pair.b = id;
    m.dispatch_collision_events(CollisionEvents {
        entered: vec![pair],
        ..Default::default()
    });
    let want = format!("-1.0,-3.0,{}", id + 100);
    assert_eq!(m.eval("__n").unwrap(), want);
}
