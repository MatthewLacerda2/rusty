//! Root motion blended as poses are (#1745): a crossfade starting mid-clip
//! hands one motion's travel to the next without a jump, a blend tree's travel
//! is its clips' weighted, and layers stack theirs by weight.

use glam::Vec3;

use super::super::step;
use super::super::test_rig::*;
use super::super::tree_tests::tree_node;
use super::tests::{run, total, DT};
use crate::asset::animation_graph::{AnimationGraph, LayerBlending};
use crate::components::{AnimatorComponent, MeshComponent, Motion};

/// Walk: hips +2 m over 1 s; Run: +6 m over 2 s; Back: hips −1 m over 1 s.
fn rig() -> MeshComponent {
    mesh(vec![
        slide("Walk", &[HIPS], 0.0, 2.0, 1.0),
        slide("Run", &[HIPS], 0.0, 6.0, 2.0),
        slide("Back", &[HIPS], 0.0, -1.0, 1.0),
    ])
}

fn steps_with(anim: &mut AnimatorComponent, mesh: &MeshComponent, g: &AnimationGraph) -> f32 {
    (0..60)
        .map(|_| step(anim, Some(mesh), Some(g), DT).root.unwrap_or_default())
        .fold(Vec3::ZERO, |sum, m| sum + m.position)
        .x
}

#[test]
fn a_crossfade_starting_mid_clip_hands_over_without_a_jump() {
    let mesh = rig();
    let mut anim = animator(Motion::Clip("Walk"), None, 0.0);
    anim.base.loop_clip = true;
    run(&mut anim, &mesh, 18);
    anim.crossfade("Back".to_string(), 0.5);
    let steps = run(&mut anim, &mesh, 60);
    let (walk, back) = (2.0 * DT, -DT);
    for pair in steps.windows(2) {
        let change = (pair[1].position.x - pair[0].position.x).abs();
        assert!(
            change <= (walk - back) * DT / 0.5 + 1e-4,
            "smooth: {pair:?}"
        );
    }
    assert!(steps[0].position.x > 0.9 * walk, "starts as the walk");
    assert!(
        (steps[59].position.x - back).abs() < 1e-4,
        "ends as the back step"
    );
}

#[test]
fn a_blend_tree_travels_its_clips_weighted_stride() {
    let mesh = rig();
    let g = graph(
        vec![tree_node("Loco", &[("Walk", 2.0), ("Run", 6.0)])],
        Vec::new(),
    );
    let mut anim = animator(Motion::Tree("Loco"), None, 0.0);
    anim.base.loop_clip = true;
    anim.set_float("speed", 4.0);
    // Half walk, half run, at the blended cycle of 1.5 s: (0.5·2 + 0.5·6) m per
    // cycle, so 4/1.5 m in one second.
    let travelled = steps_with(&mut anim, &mesh, &g);
    assert!((travelled - 4.0 / 1.5).abs() < 1e-3, "{travelled}");
}

#[test]
fn layers_stack_their_travel_by_weight() {
    let mesh = rig();
    let layered = |blending, weight: f32| {
        let upper = layer(blending, &["hips"], vec![node("Back", "Back")]);
        let g = graph(vec![node("Walk", "Walk")], vec![upper]);
        let mut anim = animator(Motion::Clip("Walk"), Some(Motion::Clip("Back")), 0.0);
        anim.layers[0].weight = weight;
        steps_with(&mut anim, &mesh, &g)
    };
    assert!((layered(LayerBlending::Override, 1.0) + 1.0).abs() < 1e-3);
    assert!((layered(LayerBlending::Override, 0.5) - 0.5).abs() < 1e-3);
    assert!((layered(LayerBlending::Additive, 1.0) - 1.0).abs() < 1e-3);
    assert!((layered(LayerBlending::Override, 0.0) - 2.0).abs() < 1e-3);
}

#[test]
fn a_layer_masked_off_the_bone_leaves_its_travel_alone() {
    let mesh = rig();
    let upper = layer(
        LayerBlending::Override,
        &["spine"],
        vec![node("Back", "Back")],
    );
    let g = graph(vec![node("Walk", "Walk")], vec![upper]);
    let mut anim = animator(Motion::Clip("Walk"), Some(Motion::Clip("Back")), 0.0);
    assert!((steps_with(&mut anim, &mesh, &g) - 2.0).abs() < 1e-3);
    let mut again = animator(Motion::Clip("Walk"), None, 0.0);
    assert!((total(&run(&mut again, &mesh, 60)).position.x - 2.0).abs() < 1e-3);
}
