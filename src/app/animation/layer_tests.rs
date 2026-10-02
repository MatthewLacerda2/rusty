//! Layers and avatar masks (#457): a masked upper-body layer leaves the legs to
//! the base layer, weights blend, an additive layer adds only its change, and a
//! layer never writes a joint it doesn't key.

use glam::{Quat, Vec3};

use super::layers::mask_slots;
use super::test_rig::*;
use super::*;
use crate::asset::animation_graph::LayerBlending::{Additive, Override};
use crate::components::Motion;

/// The x translation each joint is posed at (`None` where undriven).
fn xs(pose: &Pose) -> Vec<Option<f32>> {
    pose.iter().map(|j| j.map(|t| t.translation.x)).collect()
}

fn pose_of(anim: &AnimatorComponent, mesh: &MeshComponent, graph: &AnimationGraph) -> Pose {
    sample_pose(anim, mesh, Some(graph)).unwrap().1
}

#[test]
fn a_mask_covers_each_named_bone_and_its_subtree() {
    let skin = mesh(Vec::new()).skin.unwrap();
    let mask = |names: &[&str]| {
        mask_slots(
            &skin,
            &names.iter().map(|n| n.to_string()).collect::<Vec<_>>(),
        )
    };
    assert_eq!(mask(&["spine"]), vec![false, true, false, true]);
    assert_eq!(mask(&["leg", "head"]), vec![false, false, true, true]);
    assert_eq!(mask(&[]), vec![true; 4], "no mask is the whole body");
    assert_eq!(
        mask(&["tail"]),
        vec![false; 4],
        "an absent bone covers nothing"
    );
}

#[test]
fn a_masked_upper_body_layer_leaves_the_legs_untouched() {
    let mesh = mesh(vec![
        slide("Run", &[HIPS, SPINE, LEG, HEAD], 0.0, 2.0, 1.0),
        slide("Aim", &[HIPS, SPINE, LEG, HEAD], 0.0, -4.0, 1.0),
    ]);
    let upper = layer(Override, &["spine"], vec![node("Aim", "Aim")]);
    let graph = graph(vec![node("Run", "Run")], vec![upper]);
    let mut anim = animator(Motion::Clip("Run"), Some(Motion::Clip("Aim")), 0.5);
    // Run is at +1 everywhere, Aim at -2: the spine subtree takes Aim, the rest Run.
    assert_eq!(
        xs(&pose_of(&anim, &mesh, &graph)),
        vec![Some(1.0), Some(-2.0), Some(1.0), Some(-2.0)]
    );
    anim.layers[0].weight = 0.5;
    assert_eq!(
        xs(&pose_of(&anim, &mesh, &graph)),
        vec![Some(1.0), Some(-0.5), Some(1.0), Some(-0.5)]
    );
    anim.layers[0].weight = 0.0;
    assert_eq!(
        xs(&pose_of(&anim, &mesh, &graph)),
        vec![Some(1.0); 4],
        "weight 0 hides it"
    );
}

#[test]
fn an_additive_layer_on_its_reference_pose_is_a_no_op() {
    let mesh = mesh(vec![
        slide("Run", &[HIPS, SPINE, LEG, HEAD], 0.0, 2.0, 1.0),
        slide("Still", &[HIPS, SPINE, LEG, HEAD], 0.5, 0.5, 1.0),
    ]);
    let graph = graph(
        vec![node("Run", "Run")],
        vec![layer(Additive, &[], vec![node("Still", "Still")])],
    );
    let anim = animator(Motion::Clip("Run"), Some(Motion::Clip("Still")), 0.5);
    let base = animator(Motion::Clip("Run"), None, 0.5);
    assert_eq!(pose_of(&anim, &mesh, &graph), pose_of(&base, &mesh, &graph));
}

#[test]
fn an_additive_layer_adds_its_change_scaled_by_weight() {
    let mesh = mesh(vec![
        slide("Run", &[HIPS, SPINE, LEG, HEAD], 0.0, 2.0, 1.0),
        turn("Flinch", SPINE, 1.0),
    ]);
    let graph = graph(
        vec![node("Run", "Run")],
        vec![layer(Additive, &[], vec![node("Flinch", "Flinch")])],
    );
    let mut anim = animator(Motion::Clip("Run"), Some(Motion::Clip("Flinch")), 0.5);
    anim.layers[0].weight = 0.5;
    let pose = pose_of(&anim, &mesh, &graph);
    let spine = pose[SPINE].unwrap();
    // Half-way the flinch has turned 0.5 rad from its first frame; at half weight, 0.25.
    assert!(
        spine
            .rotation
            .abs_diff_eq(Quat::from_rotation_z(0.25), 1e-5),
        "{spine:?}"
    );
    assert!(
        spine.translation.abs_diff_eq(Vec3::X, 1e-6),
        "the base translation stays"
    );
    assert_eq!(
        pose[LEG].unwrap().rotation,
        Quat::IDENTITY,
        "unkeyed joints are not touched"
    );
}

#[test]
fn a_layer_keying_a_joint_the_base_leaves_alone_starts_from_bind() {
    let mesh = mesh(vec![
        slide("Legs", &[LEG], 0.0, 2.0, 1.0),
        slide("Aim", &[SPINE], 0.0, -4.0, 1.0),
    ]);
    let graph = graph(
        vec![node("Legs", "Legs")],
        vec![layer(Override, &[], vec![node("Aim", "Aim")])],
    );
    let mut anim = animator(Motion::Clip("Legs"), Some(Motion::Clip("Aim")), 0.5);
    anim.layers[0].weight = 0.5;
    // Only the keyed joints are written: hips and head stay undriven.
    assert_eq!(
        xs(&pose_of(&anim, &mesh, &graph)),
        vec![None, Some(-1.0), Some(1.0), None]
    );
    // Without the graph, the layer state is ignored: the base alone.
    let alone = sample_pose(&anim, &mesh, None).unwrap().1;
    assert_eq!(xs(&alone), vec![None, None, Some(1.0), None]);
}
