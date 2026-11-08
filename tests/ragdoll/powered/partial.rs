//! `SetSimulated` switches one branch, which keeps colliding with nothing.

use rusty::physics::PhysicsWorld;

use super::{at_rest, rig, step};

#[test]
fn set_simulated_on_the_upper_arm_switches_only_that_branch() {
    let (mut scene, rig) = crate::ragdoll::ragdolled();
    let arm = rig::bone(&scene, rig, "LeftArm");
    assert_eq!(scene.set_simulated(rig, arm, true, &at_rest), 2);
    let dynamic = |s: &rusty::scene::Scene, n: &str| {
        let rb = s.world.rigidbody(rig::bone(s, rig, n)).unwrap();
        !rb.is_kinematic
    };
    assert!(dynamic(&scene, "LeftArm") && dynamic(&scene, "LeftForeArm"));
    for other in ["Hips", "Spine", "Head", "RightArm", "LeftLeg"] {
        assert!(!dynamic(&scene, other), "{other} stays animated");
    }
    let hitbox = scene.hitbox_of(arm).unwrap();
    let hitbox_layer = scene.layers.index_of("Hitbox").unwrap();
    assert_eq!(scene.world.layer(hitbox), hitbox_layer, "no Ragdoll layer");
    assert_eq!(scene.set_simulated(rig, arm, false, &at_rest), 2);
    assert!(!dynamic(&scene, "LeftArm") && !dynamic(&scene, "LeftForeArm"));
    assert!(scene.ragdolls.animated(arm).is_none(), "untracked again");
}

#[test]
fn a_simulated_branch_hits_no_world_collider() {
    let (mut scene, rig) = crate::ragdoll::ragdolled();
    let fore = rig::bone(&scene, rig, "LeftForeArm");
    // A wall right through the forearm: a colliding body would be thrown out.
    let wall = rig::floor(&mut scene);
    let at = crate::ragdoll::hitbox_at(&scene, fore);
    scene.world.transform_mut(wall).unwrap().position = at;
    let arm = rig::bone(&scene, rig, "LeftArm");
    scene.set_simulated(rig, arm, true, &at_rest);
    let mut physics = PhysicsWorld::from_scene(&scene);
    for _ in 0..30 {
        let events = physics.step(&mut scene, 1.0 / 60.0);
        physics.pose_bones(&mut scene);
        assert!(
            events.collisions.stayed.is_empty(),
            "a hitbox-layer branch touches nothing"
        );
    }
    step(&mut scene, &mut physics, 1);
    let y = crate::ragdoll::hitbox_at(&scene, fore).y;
    assert!(y < at.y - 0.05, "it fell through the wall: {y} vs {}", at.y);
}
