//! The graph-state callbacks (#1901): a crossfade enters its target as it
//! starts and exits its source as it ends, both updating between; a hard cut
//! exits and enters in one step; binding enters every layer's entry node.

use super::super::graph::step_graph;
use super::super::step;
use super::super::test_rig::{graph, layer, mesh, node, slide};
use super::*;
use rusty_asset::animation_graph::{
    AnimationGraph, Condition, GraphEdge, GraphNode, LayerBlending,
};
use rusty_components::MeshComponent;

pub(super) const DT: f32 = 1.0 / 60.0;

/// Base `A → B` on `go` (a 0.1 s crossfade), `C` reached only by a jump, with
/// `A` and `C` tagged `"attack"`; a one-node layer `Aim`.
pub(super) fn rig() -> (AnimatorComponent, MeshComponent, AnimationGraph) {
    let tagged = |name: &str| GraphNode {
        tag: "attack".to_string(),
        ..node(name, name)
    };
    let mut graph = graph(
        vec![tagged("A"), node("B", "B"), tagged("C")],
        vec![layer(LayerBlending::Override, &[], vec![node("Aim", "A")])],
    );
    graph.base.edges = vec![GraphEdge {
        from: "A".to_string(),
        to: "B".to_string(),
        conditions: vec![Condition::Bool {
            parameter: "go".to_string(),
            value: true,
        }],
        transition_duration: 0.1,
    }];
    let clips = ["A", "B", "C"].map(|c| slide(c, &[0], 0.0, 1.0, 1.0));
    (AnimatorComponent::default(), mesh(clips.to_vec()), graph)
}

/// One fixed step — evaluate, advance, diff — as `Enter A@0`-style lines.
pub(super) fn tick(
    anim: &mut AnimatorComponent,
    mesh: &MeshComponent,
    graph: &AnimationGraph,
) -> Vec<String> {
    step_graph(anim, graph);
    step(anim, Some(mesh), Some(graph), DT);
    let callbacks = state_callbacks(anim, &mesh.clips, Some(graph));
    callbacks
        .iter()
        .map(|c| format!("{:?} {}@{}", c.event, c.info.state, c.info.layer))
        .collect()
}

#[test]
fn binding_enters_every_layers_entry_node() {
    let (mut anim, mesh, graph) = rig();
    assert_eq!(tick(&mut anim, &mesh, &graph), ["Enter A@0", "Enter Aim@1"]);
    assert_eq!(
        tick(&mut anim, &mesh, &graph),
        ["Update A@0", "Update Aim@1"]
    );
}

#[test]
fn a_crossfade_enters_at_its_start_and_exits_at_its_end() {
    let (mut anim, mesh, graph) = rig();
    tick(&mut anim, &mesh, &graph);
    anim.set_bool("go", true);
    let first = tick(&mut anim, &mesh, &graph);
    assert_eq!(first, ["Enter B@0", "Update A@0", "Update Aim@1"]);
    let mut between = 0;
    loop {
        let lines = tick(&mut anim, &mesh, &graph);
        if lines[0] == "Exit A@0" {
            assert_eq!(lines, ["Exit A@0", "Update B@0", "Update Aim@1"]);
            break;
        }
        assert_eq!(lines, ["Update A@0", "Update B@0", "Update Aim@1"]);
        between += 1;
        assert!(between < 10, "the 0.1 s crossfade never ended");
    }
    assert!(
        between >= 4,
        "both states update while the crossfade blends"
    );
    assert_eq!(
        tick(&mut anim, &mesh, &graph),
        ["Update B@0", "Update Aim@1"]
    );
}

#[test]
fn a_hard_cut_exits_then_enters_in_one_step_and_reentry_counts() {
    let (mut anim, mesh, graph) = rig();
    tick(&mut anim, &mesh, &graph);
    anim.enter_node(graph.node("C").unwrap(), 0.0);
    let cut = tick(&mut anim, &mesh, &graph);
    assert_eq!(cut, ["Exit A@0", "Enter C@0", "Update Aim@1"]);
    anim.enter_node(graph.node("C").unwrap(), 0.0);
    let again = tick(&mut anim, &mesh, &graph);
    assert_eq!(again, ["Exit C@0", "Enter C@0", "Update Aim@1"]);
}

#[test]
fn info_carries_the_tag_layer_and_unwrapped_time() {
    let (mut anim, mesh, graph) = rig();
    for _ in 0..90 {
        tick(&mut anim, &mesh, &graph);
    }
    let reported = &anim.states.layers[0][0].info;
    assert_eq!(reported.tag.as_deref(), Some("attack"));
    assert_eq!(reported.layer_name, BASE_LAYER);
    assert_eq!(reported.length, 1.0);
    // 90 steps of a looping 1 s clip: past 1 on its second cycle.
    assert!(
        (reported.normalized_time - 1.5).abs() < 1e-3,
        "{reported:?}"
    );
    assert!((reported.time - 1.5).abs() < 1e-3);
    assert_eq!(anim.states.layers[1][0].info.layer_name, "Upper");
    assert_eq!(anim.states.layers[1][0].info.tag, None);
}

#[test]
fn two_runs_raise_the_same_sequence() {
    let run = || {
        let (mut anim, mesh, graph) = rig();
        let mut all = Vec::new();
        for i in 0..40 {
            anim.set_bool("go", i >= 5);
            if i == 25 {
                anim.enter_node(graph.node("C").unwrap(), 0.0);
            }
            step_graph(&mut anim, &graph);
            step(&mut anim, Some(&mesh), Some(&graph), DT);
            all.extend(state_callbacks(&mut anim, &mesh.clips, Some(&graph)));
        }
        all
    };
    let first = run();
    assert!(first.len() > 40);
    assert_eq!(first, run());
}
