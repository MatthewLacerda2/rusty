use glam::{Quat, Vec3};

use super::fixture_tests::digest;
use super::ops_tests::{apply, Op, Phys, SETTLE_STEPS};
use super::tests::{agree, Twin};
use crate::PhysicsWorld;
use rusty_scene::Scene;

const DT: f32 = 1.0 / 60.0;

fn ops() -> Vec<Op> {
    use rusty_ecs::changes::ops::{Edit, Op as W};
    vec![
        Op::Settle,
        Op::On(2, Phys::Velocity(-22)),
        Op::World(W::On(1, Edit::Adopt(false))),
        Op::Step,
        Op::Step,
        Op::Settle,
        Op::World(W::On(2, Edit::SetStatic(true))),
        Op::Step,
        Op::Step,
        Op::Pause,
    ]
}

#[test]
fn a_body_turned_static_steps_alike() {
    agree(&ops()).unwrap();
}

#[test]
fn a_body_just_turned_static_is_visited_by_the_next_sync() {
    let ops = ops();
    let mut twin = Twin::new();
    for op in &ops[..8] {
        twin.run(op).unwrap();
    }
    let id = 3;
    let body = &twin.fast.bodies[twin.fast.id_to_body[&id]];
    assert!(body.is_fixed(), "entity {id} is the box turned static");
    assert!(twin.fast.tracked.stepped.contains(&id));
}

fn stack() -> (Scene, Vec<u32>) {
    let mut scene = Scene::new();
    let mut physics = PhysicsWorld::from_scene(&scene);
    for i in 0..3 {
        scene.add_entity(format!("e{i}"));
    }
    let ids = scene.world.ids().to_vec();
    for i in 1..3 {
        apply(&mut scene, &mut physics, &Op::On(i, Phys::Collider(0)));
        apply(&mut scene, &mut physics, &Op::On(i, Phys::Body(false)));
    }
    apply(&mut scene, &mut physics, &Op::On(0, Phys::Collider(2)));
    scene.world.set_static(ids[0], true);
    let w = &mut scene.world;
    w.transform_mut(ids[1]).unwrap().position = Vec3::new(0.0, 0.75, 0.0);
    w.transform_mut(ids[2]).unwrap().position = Vec3::new(0.1, 1.8, 0.0);
    (scene, ids)
}

#[test]
fn a_body_resting_on_a_frozen_prop_sleeps() {
    // A hair off unit length (its squares sum to 1 + 2e-7), so the solver has a rotation to round.
    let rot = Quat::from_xyzw(0.0, 0.92480135, 0.0, 0.3804506);
    let mut twins = [stack(), stack()].map(|(scene, ids)| {
        let mut physics = PhysicsWorld::from_scene(&scene);
        let mut scene = scene;
        for _ in 0..SETTLE_STEPS * 2 {
            physics.step(&mut scene, DT);
        }
        let w = &mut scene.world;
        w.rigidbody_mut(ids[1]).unwrap().velocity = Vec3::new(0.01, 0.0, 0.0);
        physics.step(&mut scene, DT);
        scene.world.transform_mut(ids[1]).unwrap().rotation = rot;
        scene.world.set_static(ids[1], true);
        (scene, ids, physics)
    });
    twins[1].2.tracked.always_full = true;
    for _ in 0..SETTLE_STEPS {
        for (scene, _, physics) in &mut twins {
            physics.step(scene, DT);
        }
        let [a, b] = &twins;
        assert_eq!(digest(&a.2), digest(&b.2), "rapier differs");
    }
    for (_, ids, physics) in &twins {
        let top = &physics.bodies[physics.id_to_body[&ids[2]]];
        assert!(top.is_sleeping(), "the box on the frozen prop never sleeps");
    }
}
