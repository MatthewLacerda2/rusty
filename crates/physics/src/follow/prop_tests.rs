//! The kept bone list against the walk (#2345): over random edits to a rig with
//! hitboxes (`edit_ops`), made before the step or after it, after every tick the
//! kept list equals a fresh walk, a twin that walks every tick
//! (`Tracked::always_walk`) holds bit-identical colliders, and every bone-hung
//! collider sits at its entity's world pose.

use proptest::prelude::*;

use super::cost_tests::{rig, DT};
use super::edit_ops::{apply, op};
use super::walked;
use crate::compound::world_pose;
use crate::convert::to_pose;
use crate::PhysicsWorld;
use rusty_scene::Scene;

/// The kept list is the walk's, the twin's colliders are this one's to the bit,
/// and every hung collider that is no body's own sits at its world pose.
fn check(scene: &Scene, kept: &PhysicsWorld, walking: &PhysicsWorld) -> Result<(), TestCaseError> {
    let ids = kept.tracked.follow.as_ref().unwrap().ids();
    let walk = walked(scene, &kept.id_to_collider);
    prop_assert_eq!(ids, walk.as_slice());
    prop_assert_eq!(kept.id_to_collider.len(), walking.id_to_collider.len());
    for (id, &h) in &kept.id_to_collider {
        let twin = walking.id_to_collider[id];
        prop_assert_eq!(
            kept.colliders[h].position(),
            walking.colliders[twin].position()
        );
    }
    for id in ids {
        let w = &scene.world;
        let carrier =
            kept.plan.contains_key(id) && (w.has_rigidbody(*id) || w.has_character_controller(*id));
        let (Some(pose), false) = (world_pose(scene, *id), carrier) else {
            continue;
        };
        let at = *kept.colliders[kept.id_to_collider[id]].position();
        prop_assert_eq!(
            at,
            to_pose(pose.pos, pose.rot),
            "collider {} off its bone",
            id
        );
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]
    #[test]
    fn the_kept_list_is_the_walk_after_every_tick(
        ops in proptest::collection::vec((op(), any::<bool>()), 1..24),
    ) {
        let mut scenes = [Scene::new(), Scene::new()];
        let rig = rig(&mut scenes[0], true);
        prop_assert_eq!(rig, self::rig(&mut scenes[1], true));
        let mut kept = PhysicsWorld::from_scene(&scenes[0]);
        let mut walking = PhysicsWorld::from_scene(&scenes[1]);
        walking.tracked.always_walk = true;
        // A late edit lands after the step, as a `LateUpdate` script's does.
        for (op, late) in &ops {
            let [a, b] = &mut scenes;
            for (physics, scene) in [(&mut kept, a), (&mut walking, b)] {
                if !late {
                    apply(scene, rig, op);
                }
                physics.step(scene, DT);
                if *late {
                    apply(scene, rig, op);
                }
                scene.build_skin_palettes();
                physics.follow_bones(scene);
            }
            check(&scenes[0], &kept, &walking)?;
        }
    }
}
