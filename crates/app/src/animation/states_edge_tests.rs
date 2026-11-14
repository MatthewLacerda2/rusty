//! The graph-state callbacks' edges (#1901): states that start in one step
//! enter in the order they became active, and a non-looping state holds at
//! exactly its end.

use super::states_tests::{rig, tick};
use rusty_asset::animation_graph::{Condition, GraphEdge};
use rusty_components::Motion;

#[test]
fn a_cut_and_a_crossfade_in_one_step_enter_in_the_order_they_began() {
    let (mut anim, mesh, mut graph) = rig();
    let upper = &mut graph.layers[0].machine;
    upper.nodes.push(super::super::test_rig::node("Aim2", "B"));
    upper.edges = vec![GraphEdge {
        from: "Aim".to_string(),
        to: "Aim2".to_string(),
        conditions: vec![Condition::Bool {
            parameter: "go".to_string(),
            value: true,
        }],
        transition_duration: 0.1,
    }];
    anim.set_bool("go", true);
    // The layer binds its entry `Aim` and crossfades out of it at once: `Aim`
    // entered first, so the last state entered is the current one.
    let first = tick(&mut anim, &mesh, &graph);
    assert_eq!(first, ["Enter A@0", "Enter Aim@1", "Enter Aim2@1"]);
    let next = tick(&mut anim, &mesh, &graph);
    assert_eq!(next[2..], ["Update Aim@1", "Update Aim2@1"], "{next:?}");
}

#[test]
fn a_non_looping_state_holds_at_exactly_its_end() {
    let (mut anim, mesh, mut graph) = rig();
    graph.base.nodes[0].is_loop = false;
    for _ in 0..90 {
        tick(&mut anim, &mesh, &graph);
    }
    assert_eq!(anim.base.motion(), Motion::Clip("A"));
    let reported = &anim.states.layers[0][0].info;
    assert_eq!(reported.state, "A");
    assert_eq!(reported.normalized_time, 1.0, "{reported:?}");
    assert_eq!(reported.time, 1.0);
    assert_eq!(reported.length, 1.0);
}
