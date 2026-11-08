//! The blend weight draws a simulated bone between its animated pose and its
//! body's.

use rusty::physics::PhysicsWorld;

use super::{at_rest, rig, step};

#[test]
fn at_weight_half_the_bone_sits_halfway_between_body_and_animation() {
    let (mut scene, rig) = crate::ragdoll::ragdolled();
    let arm = rig::bone(&scene, rig, "LeftArm");
    scene.set_simulated(rig, arm, true, &at_rest);
    let mut physics = PhysicsWorld::from_scene(&scene);
    step(&mut scene, &mut physics, 20); // the limp arm drops
    let (anim_pos, anim_rot) = scene.ragdolls.animated(arm).unwrap();
    let local = |s: &rusty::scene::Scene| {
        let t = s.world.transform(arm).unwrap();
        (t.position, t.rotation)
    };
    physics.pose_bones(&mut scene);
    let (body_pos, body_rot) = local(&scene);
    let off = anim_rot.angle_between(body_rot);
    assert!(off > 0.3, "the arm fell well off the animation: {off}");
    assert_eq!(scene.set_blend_weight(rig, arm, 0.5), 2);
    physics.pose_bones(&mut scene);
    let (pos, rot) = local(&scene);
    let half = anim_rot.angle_between(rot);
    assert!((half - off / 2.0).abs() < 1e-3, "{half} vs {off} / 2");
    assert!((rot.angle_between(body_rot) - off / 2.0).abs() < 1e-3);
    assert!(pos.distance(anim_pos.lerp(body_pos, 0.5)) < 1e-5);
    scene.set_blend_weight(rig, arm, 0.0);
    physics.pose_bones(&mut scene);
    assert_eq!(
        local(&scene),
        (anim_pos, anim_rot),
        "weight 0 is the animation"
    );
}
