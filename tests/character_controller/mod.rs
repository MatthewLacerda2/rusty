//! CharacterController: collide-and-slide `Move` against static geometry.

mod crouch;
mod dynamic_steps;
mod flags;
mod flat_walk;
mod hits;
mod platform;
mod push;
mod stairs;
mod steep;
mod walls;

use glam::{Quat, Vec3};
use rusty::components::{CharacterControllerComponent, ColliderComponent, ColliderShape};
use rusty::physics::{self, CharacterMove, PhysicsWorld};
use rusty::scene::Scene;

const DT: f32 = 1.0 / 60.0;
/// The scripted gravity the tests' "player" applies each tick (m/s²).
const GRAVITY: f32 = 20.0;

fn add_box(scene: &mut Scene, pos: Vec3, size: Vec3, rot: Quat) -> u32 {
    let id = scene.add_entity("Box".to_string());
    scene.world.set_static(id, true);
    let mut t = scene.world.transform_mut(id).unwrap();
    t.position = pos;
    t.rotation = rot;
    drop(t);
    let collider = ColliderComponent {
        shape: ColliderShape::Box { size },
        ..rusty::edit::authoring::default_collider()
    };
    scene.world.set_collider(id, Some(collider));
    id
}

fn add_floor(scene: &mut Scene) -> u32 {
    let size = Vec3::new(40.0, 1.0, 40.0);
    add_box(scene, Vec3::new(0.0, -0.5, 0.0), size, Quat::IDENTITY)
}

fn add_character(scene: &mut Scene, feet: Vec3) -> u32 {
    let id = scene.add_entity("Character".to_string());
    let cc = CharacterControllerComponent::default();
    let lift = cc.height * 0.5 + cc.skin_width;
    scene.world.transform_mut(id).unwrap().position = feet + Vec3::Y * lift;
    scene.world.set_character_controller(id, Some(cc));
    id
}

/// A walking script's frame: horizontal `walk` plus its own gravity, carried in
/// `vy` (reset while grounded, as a Unity script does). Moves then steps physics.
fn walk(
    physics: &mut PhysicsWorld,
    scene: &mut Scene,
    id: u32,
    walk: Vec3,
    vy: &mut f32,
) -> CharacterMove {
    *vy -= GRAVITY * DT;
    let motion = walk * DT + Vec3::Y * *vy * DT;
    let hit = physics::move_character(Some(physics), scene, id, motion).unwrap();
    if hit.grounded && *vy < 0.0 {
        *vy = -1.0;
    }
    physics.step(scene, DT);
    hit
}

fn walk_for(
    physics: &mut PhysicsWorld,
    scene: &mut Scene,
    id: u32,
    velocity: Vec3,
    ticks: usize,
) -> CharacterMove {
    let mut vy = 0.0;
    let mut last = None;
    for _ in 0..ticks {
        last = Some(walk(physics, scene, id, velocity, &mut vy));
    }
    last.unwrap()
}

fn pos_of(scene: &Scene, id: u32) -> Vec3 {
    scene.world.transform(id).unwrap().position
}

fn feet_of(scene: &Scene, id: u32) -> Vec3 {
    let h = scene.world.character_controller(id).unwrap().height;
    pos_of(scene, id) - Vec3::Y * h * 0.5
}

fn scripted_run() -> Vec3 {
    let mut scene = Scene::new();
    add_floor(&mut scene);
    add_box(
        &mut scene,
        Vec3::new(3.0, 0.1, 0.0),
        Vec3::new(2.0, 0.2, 4.0),
        Quat::IDENTITY,
    );
    add_box(
        &mut scene,
        Vec3::new(8.0, 1.0, 1.0),
        Vec3::new(1.0, 2.0, 6.0),
        Quat::IDENTITY,
    );
    let id = add_character(&mut scene, Vec3::ZERO);
    let mut physics = PhysicsWorld::from_scene(&scene);
    walk_for(&mut physics, &mut scene, id, Vec3::new(4.0, 0.0, 1.0), 150);
    walk_for(&mut physics, &mut scene, id, Vec3::new(-3.0, 0.0, -2.0), 60);
    pos_of(&scene, id)
}

#[test]
fn moves_are_deterministic_across_runs() {
    let (a, b) = (scripted_run(), scripted_run());
    assert_eq!(
        a.to_array().map(f32::to_bits),
        b.to_array().map(f32::to_bits)
    );
}
