//! A Play world's result is a function of its own scene and inputs, not of what
//! the process ran before it (#2076): the editor plays a scene after earlier
//! Play sessions, a test binary or `make game-test` after other worlds. The
//! same scene, played first in the process and again after a different world,
//! walks to bit-identical poses.
//!
//! The scene is #2061's: a static plane (a triangle-mesh floor) under
//! script-walked CharacterControllers, with a dynamic Rigidbody box dropped
//! beside them. The world between is another one: kinematic box floor, more
//! walkers, other speeds.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use rusty::components::{CharacterControllerComponent, ColliderShape};
use rusty::scene::authoring::{add_component, create_entity, ComponentKind, Primitive};
use rusty::scene::Scene;

use crate::play::{attach_script, playing, run};

const TICKS: usize = 120;

/// A floor named `name`, its top face at y = 0, 60 m along +Z.
fn add_floor(scene: &mut Scene, name: &str, primitive: Primitive, is_static: bool) {
    let floor = create_entity(scene, name, Some(primitive));
    let mut t = scene.world.transform_mut(floor).unwrap();
    let plane = primitive == Primitive::Plane;
    t.position = Vec3::new(0.0, if plane { 0.0 } else { -0.5 }, 20.0);
    t.scale = if plane {
        Vec3::new(4.0, 1.0, 4.0)
    } else {
        Vec3::new(60.0, 1.0, 60.0)
    };
    drop(t);
    assert!(add_component(scene, floor, ComponentKind::Collider));
    scene.world.set_static(floor, is_static);
    if plane {
        // The plane's collider is its mesh (the default added is a unit box).
        let (local_min, local_max) = (Vec3::new(-7.5, 0.0, -7.5), Vec3::new(7.5, 0.0, 7.5));
        let shape = ColliderShape::Mesh {
            convex: false,
            local_min,
            local_max,
        };
        scene.world.collider_mut(floor).unwrap().shape = shape;
    }
}

/// `count` controllers 2 m apart whose scripts walk them +Z, the n-th at
/// `speed * (n + 1)`, with a ground stick.
fn add_walkers(scene: &mut Scene, count: usize, speed: f32) -> Vec<u32> {
    (0..count)
        .map(|n| {
            let id = scene.add_entity("Walker".to_string());
            let cc = CharacterControllerComponent::default();
            let y = cc.height * 0.5 + cc.skin_width;
            let at = Vec3::new(-8.0 + 2.0 * n as f32, y, -5.0);
            scene.world.transform_mut(id).unwrap().position = at;
            scene.world.set_character_controller(id, Some(cc));
            let v = speed * (n + 1) as f32;
            let source = format!(
                "local W = {{}}\nfunction W.Update(id, dt)\n  \
                 CharacterController.Move(id, 0.1 * dt, -0.02, {v} * dt)\nend\nreturn W\n"
            );
            attach_script(scene, id, &format!("play_order_{speed}_{n}.lua"), &source);
            id
        })
        .collect()
}

/// Play `scene` for `TICKS`, returning `ids`' positions every tick, as bits.
fn play(scene: Scene, ids: &[u32]) -> Vec<[u32; 3]> {
    let scene = Rc::new(RefCell::new(scene));
    let mut game = playing(scene.clone(), 100.0);
    let mut poses = Vec::new();
    for _ in 0..TICKS {
        run(&mut game, 1);
        let s = scene.borrow();
        for &id in ids {
            let p = s.world.transform(id).unwrap().position;
            poses.push([p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]);
        }
    }
    poses
}

/// The scene under test: a static plane, walkers and a falling dynamic box.
fn plane_scene() -> Vec<[u32; 3]> {
    let mut scene = Scene::new();
    add_floor(&mut scene, "Plane", Primitive::Plane, true);
    let mut ids = add_walkers(&mut scene, 6, 2.0);
    let crate_id = create_entity(&mut scene, "Crate", Some(Primitive::Box));
    scene.world.transform_mut(crate_id).unwrap().position = Vec3::new(4.0, 3.0, 0.0);
    assert!(add_component(&mut scene, crate_id, ComponentKind::Collider));
    let rigidbody = ComponentKind::parse("Rigidbody").unwrap();
    assert!(add_component(&mut scene, crate_id, rigidbody));
    ids.push(crate_id);
    play(scene, &ids)
}

/// A different world: a kinematic box floor under more, faster walkers.
fn walking_world() {
    let mut scene = Scene::new();
    add_floor(&mut scene, "Floor", Primitive::Box, false);
    let ids = add_walkers(&mut scene, 16, 5.0);
    play(scene, &ids);
}

#[test]
fn a_scene_plays_the_same_after_another_world() {
    let first = plane_scene();
    walking_world();
    let again = plane_scene();
    let at = first.iter().zip(&again).position(|(a, b)| a != b);
    assert_eq!(at, None, "pose {at:?} of {} differs", first.len());
    assert_eq!(first.len(), again.len());
    // Not a still scene: the crate fell and the first walker crossed the plane.
    let (walker, crate_pose) = (first[first.len() - 7], first[first.len() - 1]);
    assert!(f32::from_bits(crate_pose[1]) < 2.0, "the crate never fell");
    assert!(f32::from_bits(walker[2]) > -1.5, "the walker never walked");
}
