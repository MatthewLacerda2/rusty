//! src/physics/watch/stepped_tests.rs — a body the step changed that no change
//! named (#1842): rapier's solver rewrites the pose of every body in its active
//! set, a body just turned static included, and no write-back stamps a static
//! body, so the incremental sync must visit it anyway, and must not re-push it for
//! the solver's rounding, which would keep what rests on it awake.

use glam::{Quat, Vec3};

use super::fixture_tests::digest;
use super::ops_tests::{apply, Op, Phys, SETTLE_STEPS};
use super::tests::{agree, Twin};
use crate::physics::PhysicsWorld;
use crate::scene::Scene;

const DT: f32 = 1.0 / 60.0;

/// The sequence that drifted (#1842): an inactive parent adopts entity 1, a
/// settled box (entity 3) turns static, two steps, then a paused one. The full
/// pass re-pushed the box's rotation on the paused step, the incremental one left
/// the solver's.
fn ops() -> Vec<Op> {
    use crate::ecs::changes::ops::{Edit, Op as W};
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

/// After the step that follows `SetStatic`, the box is Fixed and in the set the
/// next sync visits whether a change names it or not. Whether the solver hands its
/// rotation back an ulp off is the solver's rounding, which differs by platform
/// (aarch64 returns it unchanged), so it is not asserted.
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

/// A static ground, a dynamic box on it and a second box dropped onto the first.
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

/// A body resting on a prop frozen while awake falls asleep, on both paths, and
/// they agree (#1842's review). The prop stays in rapier's active set, and its
/// rotation came back rounded every step: compared with rapier's pose, it was
/// re-pushed every step, which woke the box on it for good (300 pushes in 300
/// steps on x86_64; `Tracked::static_poses` makes it one).
/// The prop is frozen with a rotation a hair off unit length, which the solver
/// rounds on x86_64.
#[test]
fn a_body_resting_on_a_frozen_prop_sleeps() {
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
