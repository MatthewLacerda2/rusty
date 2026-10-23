//! A ragdoll built after a bone was destroyed (#1273): the missing bone is no
//! direction to point in, so its parent's joint axis is the same as on a whole rig.

use glam::Vec3;
use rusty::scene::skeleton::RagdollOptions;
use rusty::scene::Scene;

use super::rig;

/// Spine's joint axis on a freshly built ragdoll, its head destroyed first or not.
fn spine_axis(decapitate: bool) -> Vec3 {
    let mut scene = Scene::new();
    let rig = rig::character(&mut scene, Vec3::ZERO, false);
    if decapitate {
        scene.destroy_entity(rig::bone(&scene, rig, "Head"));
    }
    scene
        .build_ragdoll(rig, &RagdollOptions::default())
        .unwrap();
    let spine = rig::bone(&scene, rig, "Spine");
    let axis = scene.world.joint(spine).unwrap().axis;
    axis
}

#[test]
fn a_ragdoll_built_after_a_decapitation_keeps_the_spine_axis_along_the_body() {
    let whole = spine_axis(false);
    let decapitated = spine_axis(true);
    assert!(
        decapitated.abs_diff_eq(whole, 1e-4),
        "decapitated {decapitated}, whole {whole}"
    );
}
