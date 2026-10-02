//! The evaluator over layers (#457): every layer binds to its entry at its
//! authored weight, steps its own machine over the shared parameters, and a
//! trigger read by several layers fires in each before it is cleared.

use super::super::test_rig::{graph, layer, node};
use super::*;
use crate::asset::animation_graph::{LayerBlending, ParameterDeclaration};

/// Base `Run → Stop` and layer `Aim → Shoot`, both on the trigger `Fire`; the
/// layer also goes `Aim → Reload` when `ammo` drops to 0.
fn armed_graph() -> AnimationGraph {
    let fire = || {
        vec![Condition::Trigger {
            parameter: "Fire".to_string(),
        }]
    };
    let edge = |from: &str, to: &str, conditions| GraphEdge {
        from: from.to_string(),
        to: to.to_string(),
        conditions,
        transition_duration: 0.1,
    };
    let mut upper = layer(
        LayerBlending::Override,
        &["spine"],
        vec![
            node("Aim", "Aim"),
            node("Shoot", "Shoot"),
            node("Reload", "Reload"),
        ],
    );
    upper.weight = 0.6;
    upper.machine.edges = vec![
        edge("Aim", "Shoot", fire()),
        edge(
            "Aim",
            "Reload",
            vec![Condition::Float {
                parameter: "ammo".to_string(),
                op: NumericOp::LessEqual,
                value: 0.0,
            }],
        ),
    ];
    let mut graph = graph(vec![node("Run", "Run"), node("Stop", "Stop")], vec![upper]);
    graph.base.edges = vec![edge("Run", "Stop", fire())];
    graph.parameters.extend([
        ("Fire".to_string(), ParameterDeclaration::Trigger),
        ("ammo".to_string(), ParameterDeclaration::Float(30.0)),
    ]);
    graph
}

fn node_of(anim: &AnimatorComponent, layer: usize) -> Option<&str> {
    match layer {
        0 => anim.base.current_node.as_deref(),
        i => anim.layers[i - 1].playback.current_node.as_deref(),
    }
}

#[test]
fn the_first_step_binds_every_layer_at_its_entry_and_weight() {
    let graph = armed_graph();
    let mut anim = AnimatorComponent::default();
    step_graph(&mut anim, &graph);
    assert_eq!(node_of(&anim, 0), Some("Run"));
    assert_eq!(node_of(&anim, 1), Some("Aim"));
    assert_eq!(anim.layers[0].name, "Upper");
    assert_eq!(anim.layers[0].weight, 0.6);
    assert_eq!(anim.layer_index("Upper"), Some(1));
    // A weight a script changed survives later steps.
    anim.layers[0].weight = 0.2;
    step_graph(&mut anim, &graph);
    assert_eq!(anim.layers[0].weight, 0.2);
}

#[test]
fn layers_step_their_own_machines_over_shared_parameters() {
    let graph = armed_graph();
    let mut anim = AnimatorComponent::default();
    step_graph(&mut anim, &graph);
    anim.set_float("ammo", 0.0);
    step_graph(&mut anim, &graph);
    assert_eq!(node_of(&anim, 0), Some("Run"), "the base doesn't read ammo");
    assert_eq!(node_of(&anim, 1), Some("Reload"));
    assert!(anim.layers[0].playback.is_crossfading());
}

#[test]
fn one_trigger_fires_in_every_layer_that_reads_it_then_clears() {
    let graph = armed_graph();
    let mut anim = AnimatorComponent::default();
    step_graph(&mut anim, &graph);
    anim.set_trigger("Fire");
    step_graph(&mut anim, &graph);
    assert_eq!(node_of(&anim, 0), Some("Stop"));
    assert_eq!(node_of(&anim, 1), Some("Shoot"));
    assert!(
        !anim.trigger_is_set("Fire"),
        "consumed once all layers decided"
    );
}

#[test]
fn a_layer_whose_graph_changed_rebinds_and_extra_states_drop() {
    let mut graph = armed_graph();
    let mut anim = AnimatorComponent::default();
    step_graph(&mut anim, &graph);
    graph.layers[0].machine.nodes.retain(|n| n.name != "Aim");
    graph.layers[0].machine.edges.clear();
    graph.layers[0].machine.entry = "Shoot".to_string();
    step_graph(&mut anim, &graph);
    assert_eq!(
        node_of(&anim, 1),
        Some("Shoot"),
        "the vanished node re-enters the entry"
    );
    assert_eq!(node_of(&anim, 0), Some("Run"), "the base stays bound");
    graph.layers.clear();
    step_graph(&mut anim, &graph);
    assert!(anim.layers.is_empty());
}
