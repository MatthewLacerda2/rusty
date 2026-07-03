//! Validation tests for the `AnimationGraph` asset (#315): dangling entry/edge/
//! parameter references and type-mismatched condition operators are rejected, and
//! every violation is reported in one pass.

use super::tests::sample_graph;
use super::*;

/// The violations of `graph`, flattened to one string for `contains` checks.
fn problems(graph: &AnimationGraph) -> String {
    match graph.validate() {
        Err(GraphError::Invalid(problems)) => problems.join("; "),
        other => panic!("expected Invalid, got {other:?}"),
    }
}

#[test]
fn the_representative_graph_is_valid() {
    assert_eq!(sample_graph().validate(), Ok(()));
}

#[test]
fn dangling_entry_is_rejected_including_the_empty_graph() {
    let mut graph = sample_graph();
    graph.entry = "sprint".into();
    assert!(problems(&graph).contains("entry node 'sprint' does not exist"));
    // No nodes at all ⇒ the entry necessarily dangles.
    graph.nodes.clear();
    graph.edges.clear();
    assert!(problems(&graph).contains("does not exist"));
}

#[test]
fn duplicate_or_empty_node_names_and_empty_clips_are_rejected() {
    let mut graph = sample_graph();
    graph.nodes[1].name = "idle".into();
    assert!(problems(&graph).contains("duplicate node name 'idle'"));

    let mut graph = sample_graph();
    graph.nodes[2].name = String::new();
    graph.nodes[2].clip = String::new();
    let report = problems(&graph);
    assert!(report.contains("empty name"));
    assert!(report.contains("empty clip name"));
}

#[test]
fn edges_must_join_existing_nodes() {
    let mut graph = sample_graph();
    graph.edges[0].from = "sprint".into();
    graph.edges[2].to = "fall".into();
    let report = problems(&graph);
    assert!(report.contains("edge #0 ('sprint' -> 'run') references missing node 'sprint'"));
    assert!(report.contains("edge #2 ('idle' -> 'fall') references missing node 'fall'"));
}

#[test]
fn negative_or_non_finite_durations_and_speeds_are_rejected() {
    let mut graph = sample_graph();
    graph.edges[0].transition_duration = -0.1;
    graph.edges[1].transition_duration = f32::NAN;
    graph.nodes[1].speed = Some(f32::INFINITY);
    let report = problems(&graph);
    assert!(report.contains("edge #0 ('idle' -> 'run') has a negative or non-finite duration"));
    assert!(report.contains("edge #1 ('run' -> 'idle') has a negative or non-finite duration"));
    assert!(report.contains("node 'run' has a non-finite speed"));
}

#[test]
fn conditions_on_undeclared_parameters_are_rejected() {
    let mut graph = sample_graph();
    graph.parameters.remove("Jump");
    assert!(problems(&graph)
        .contains("edge #2 ('idle' -> 'jump') conditions on undeclared parameter 'Jump'"));
}

#[test]
fn type_mismatched_conditions_are_rejected_for_every_kind() {
    // Rebind each conditioned name to a different declared type: the operator set
    // is keyed by type, so every condition on it must now fail.
    let mut graph = sample_graph();
    graph
        .parameters
        .insert("speed".into(), ParameterDeclaration::Bool(false));
    graph
        .parameters
        .insert("isGrounded".into(), ParameterDeclaration::Trigger);
    graph
        .parameters
        .insert("weapon".into(), ParameterDeclaration::Float(0.0));
    graph
        .parameters
        .insert("Jump".into(), ParameterDeclaration::Int(0));
    let report = problems(&graph);
    for name in ["speed", "isGrounded", "weapon", "Jump"] {
        assert!(
            report.contains(&format!(
                "conditions on parameter '{name}' with a type-mismatched operator"
            )),
            "missing mismatch report for '{name}': {report}"
        );
    }
}

#[test]
fn all_violations_are_collected_in_one_pass() {
    let mut graph = sample_graph();
    graph.entry = "sprint".into();
    graph.nodes[0].clip = String::new();
    graph.edges[0].to = "fall".into();
    graph.parameters.remove("Jump");
    match graph.validate() {
        Err(GraphError::Invalid(problems)) => assert_eq!(problems.len(), 4, "{problems:?}"),
        other => panic!("expected Invalid, got {other:?}"),
    }
}
