//! Animation runtime end-to-end (#80, #453): a playing animator poses the bone
//! GameObjects through the real `GameWorld::tick`, the mesh is skinned from them,
//! a child of a bone follows it, a `LateUpdate` script overrides the Animator before
//! skinning reads the bones, and the whole path is deterministic.

mod rig;

use glam::Vec3;
use rig::{armed_scene, play};

#[test]
fn the_animator_poses_the_bone_and_the_skin_follows_it() {
    let (scene, hero, hand, gun) = armed_scene(None);
    let scene = play(scene, 31); // the first tick enters Play; 30 more = 0.5 s
    let s = scene.borrow();
    let x = s.world.transform(hand).unwrap().position.x;
    assert!((x - 1.0).abs() < 1e-3, "half-way the hand slid +1, got {x}");
    // The palette is built from the bone, not sampled on the side.
    let palette = s.world.mesh(hero).unwrap().pose_palette.clone();
    assert!((palette[0].w_axis.x - x).abs() < 1e-6);
    // The gun under the hand rides along.
    let gun_at = s.compute_world_matrix(gun).w_axis.truncate();
    assert!(
        gun_at.abs_diff_eq(Vec3::new(x, 0.5, 0.0), 1e-5),
        "gun at {gun_at:?}"
    );
}

#[test]
fn a_late_update_script_overrides_the_animator_before_skinning() {
    let script = "return { LateUpdate = function(id)\n\
                  Transform.SetPosition(Animator.GetBone(id, 'hand'), 0, 3, 0)\nend }";
    let (scene, hero, _, _) = armed_scene(Some(script));
    let scene = play(scene, 10);
    let palette = scene
        .borrow()
        .world
        .mesh(hero)
        .unwrap()
        .pose_palette
        .clone();
    assert!(
        palette[0]
            .w_axis
            .truncate()
            .abs_diff_eq(Vec3::new(0.0, 3.0, 0.0), 1e-6),
        "skinning must read the LateUpdate pose, got {:?}",
        palette[0].w_axis
    );
}

#[test]
fn bone_posing_is_deterministic_across_runs() {
    let run = || {
        let (scene, hero, _, _) = armed_scene(None);
        let scene = play(scene, 45);
        let s = scene.borrow();
        let palette = s.world.mesh(hero).unwrap().pose_palette.clone();
        palette
            .iter()
            .map(|m| m.to_cols_array())
            .collect::<Vec<_>>()
    };
    assert_eq!(run(), run(), "same inputs must produce identical bytes");
}
