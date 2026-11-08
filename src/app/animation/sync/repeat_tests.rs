//! A follower leaf with a repeated pair of markers (#1896): a two-stride Run
//! under a one-stride Walk in a marker-synced tree stays on the stride it is
//! on, one stride per Walk cycle, so neither its pose nor its root motion
//! jumps a stride once a cycle.

use super::super::test_rig::*;
use super::super::{sample_pose, step};
use super::rig::*;
use crate::components::{stored_leaf, wrap_delta, AnimatorParameter};

#[test]
fn a_two_stride_follower_plays_on_without_jumping_a_stride() {
    let run = marks(&[
        ("FootL", 0.0),
        ("FootR", 0.15),
        ("FootL", 0.3),
        ("FootR", 0.45),
    ]);
    let (mesh, graph) = (
        hips_mesh(),
        graph(vec![tree(walk_marks(), run)], Vec::new()),
    );
    let mut anim = bound(&graph);
    // Walk weighs 0.75 and leads; Run follows at 0.25.
    anim.parameters
        .insert("speed".to_string(), AnimatorParameter::Float(0.25));
    let (mut last, mut halves) = (None, [false; 2]);
    for i in 0..120 {
        let root = step(&mut anim, Some(&mesh), Some(&graph), DT).root;
        let x = root.unwrap_or_default().position.x;
        assert!((0.0..0.05).contains(&x), "step {i}: root {x}");
        let t = stored_leaf(&anim.base.sync.leaves, "Run").expect("Run tracked");
        if let Some(last) = last {
            let moved = wrap_delta(t - last, RUN);
            assert!((0.0..0.05).contains(&moved), "step {i}: {last} -> {t}");
        }
        last = Some(t);
        halves[usize::from(t >= 0.3)] = true;
        let leg = sample_pose(&anim, &mesh, Some(&graph)).unwrap().1[LEG];
        let x = leg.unwrap().translation.x;
        assert!((x - 0.25 * t / RUN).abs() < 1e-4, "step {i}: pose at {x}");
    }
    assert_eq!(halves, [true, true], "both strides played");
}
