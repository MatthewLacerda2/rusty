//! Pushing dynamic bodies (#1879): a CharacterController pushes a 10 kg box
//! only with the bundled `controller_push.lua` beside it (Unity's documented
//! pattern), never on its own; a kinematic Rigidbody pushes one by itself.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use rusty::components::{ColliderComponent, ColliderShape};
use rusty::scene::authoring::{default_collider, default_rigidbody};
use rusty::scene::Scene;

use super::hits::attach_scripts;
use super::{add_character, add_floor, pos_of};
use crate::play::{playing, run};

/// Walks +X at 5 m/s with its own gravity, as the bundled player does (a slow
/// walk pressed down hard reads as pushing down: `moveDirection.y < -0.3`).
const WALKER: &str = r#"
local W = { vy = 0 }
function W.Update(id, dt)
  if CharacterController.IsGrounded(id) and W.vy < 0 then W.vy = -1 end
  W.vy = W.vy - 20 * dt
  CharacterController.Move(id, 5 * dt, W.vy * dt, 0)
end
return W
"#;

/// A 1 m, 10 kg dynamic box resting on the floor at `x`; kinematic when asked.
fn crate_at(scene: &mut Scene, x: f32, kinematic: bool) -> u32 {
    let id = scene.add_entity("Crate".to_string());
    scene.world.transform_mut(id).unwrap().position = Vec3::new(x, 0.5, 0.0);
    let collider = ColliderComponent {
        shape: ColliderShape::Box { size: Vec3::ONE },
        ..default_collider()
    };
    scene.world.set_collider(id, Some(collider));
    let body = rusty::components::RigidBodyComponent {
        mass: 10.0,
        is_kinematic: kinematic,
        ..default_rigidbody()
    };
    scene.world.set_rigidbody(id, Some(body));
    id
}

/// How far a crate 2 m ahead of a walking controller moved in 2 s.
fn crate_travel(push: bool) -> f32 {
    let mut scene = Scene::new();
    add_floor(&mut scene);
    let cc = add_character(&mut scene, Vec3::ZERO);
    let boxed = crate_at(&mut scene, 2.0, false);
    let source = include_str!("../../engine/scripts/controller_push.lua");
    let mut scripts = vec![("walker.lua", WALKER)];
    if push {
        scripts.push(("controller_push.lua", source));
    }
    attach_scripts(&mut scene, cc, &scripts);
    let scene = Rc::new(RefCell::new(scene));
    let mut game = playing(scene.clone(), 20.0);
    run(&mut game, 120);
    let x = pos_of(&scene.borrow(), boxed).x;
    x - 2.0
}

#[test]
fn the_push_script_moves_a_box_the_controller_walks_into() {
    let travel = crate_travel(true);
    assert!(travel > 1.5, "pushed along the walk: {travel}");
}

#[test]
fn without_the_script_the_controller_pushes_nothing() {
    let travel = crate_travel(false);
    assert!(travel.abs() < 0.01, "the box stays put: {travel}");
}

/// A kinematic Rigidbody moved by its Transform shoves a dynamic box out of
/// its way (the solver does it): what the controller does not do on its own.
#[test]
fn a_kinematic_rigidbody_pushes_a_dynamic_box() {
    let mut scene = Scene::new();
    add_floor(&mut scene);
    let mover = crate_at(&mut scene, 0.0, true);
    scene.world.transform_mut(mover).unwrap().position.y = 0.6;
    let boxed = crate_at(&mut scene, 2.0, false);
    let scene = Rc::new(RefCell::new(scene));
    let mut game = playing(scene.clone(), 20.0);
    for _ in 0..120 {
        let mut s = scene.borrow_mut();
        s.world.transform_mut(mover).unwrap().position.x += 3.0 / 60.0;
        drop(s);
        run(&mut game, 1);
    }
    let travel = pos_of(&scene.borrow(), boxed).x - 2.0;
    assert!(travel > 2.0, "shoved ahead of the mover: {travel}");
}
