use std::collections::BTreeSet;
use std::fmt;

use super::{AnimationGraph, Condition, ParameterDeclaration, StateMachine};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphError {
    Io(String),
    Parse(String),
    Invalid(Vec<String>),
}

impl fmt::Display for GraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GraphError::Io(s) => write!(f, "animation graph io error: {s}"),
            GraphError::Parse(s) => write!(f, "failed to parse animation graph: {s}"),
            GraphError::Invalid(problems) => {
                write!(f, "invalid animation graph: {}", problems.join("; "))
            }
        }
    }
}

impl std::error::Error for GraphError {}

impl AnimationGraph {
    /// A base-layer problem is reported bare; an extra layer's carries a `layer 'name': ` prefix.
    pub fn validate(&self) -> Result<(), GraphError> {
        let mut problems = Vec::new();
        self.check_machine("", &self.base, &mut problems);
        self.check_layers(&mut problems);
        self.check_slots(&mut problems);
        self.check_montages(&mut problems);
        if problems.is_empty() {
            Ok(())
        } else {
            Err(GraphError::Invalid(problems))
        }
    }

    fn check_machine(&self, prefix: &str, machine: &StateMachine, problems: &mut Vec<String>) {
        let mut found = Vec::new();
        self.check_nodes(machine, &mut found);
        if machine.node(&machine.entry).is_none() {
            found.push(format!("entry node '{}' does not exist", machine.entry));
        }
        self.check_edges(machine, &mut found);
        problems.extend(found.into_iter().map(|p| format!("{prefix}{p}")));
    }

    fn check_layers(&self, problems: &mut Vec<String>) {
        let mut seen = BTreeSet::new();
        for (i, layer) in self.layers.iter().enumerate() {
            let label = format!("layer #{} '{}'", i + 1, layer.name);
            if layer.name.is_empty() {
                problems.push(format!("{label} has an empty name"));
            } else if !seen.insert(layer.name.as_str()) {
                problems.push(format!("duplicate layer name '{}'", layer.name));
            }
            if !(0.0..=1.0).contains(&layer.weight) {
                problems.push(format!("{label} has a weight outside [0, 1]"));
            }
            if layer.mask.iter().any(String::is_empty) {
                problems.push(format!("{label} masks an empty bone name"));
            }
            self.check_machine(&format!("{label}: "), &layer.machine, problems);
        }
    }

    fn check_nodes(&self, machine: &StateMachine, problems: &mut Vec<String>) {
        let mut seen = BTreeSet::new();
        for node in &machine.nodes {
            if node.name.is_empty() {
                problems.push("a node has an empty name".to_string());
            }
            if !seen.insert(node.name.as_str()) {
                problems.push(format!("duplicate node name '{}'", node.name));
            }
            self.check_motion(node, problems);
            if node.speed.is_some_and(|s| !s.is_finite()) {
                problems.push(format!("node '{}' has a non-finite speed", node.name));
            }
        }
    }

    fn check_edges(&self, machine: &StateMachine, problems: &mut Vec<String>) {
        for (i, edge) in machine.edges.iter().enumerate() {
            let label = format!("edge #{i} ('{}' -> '{}')", edge.from, edge.to);
            for end in [&edge.from, &edge.to] {
                if machine.node(end).is_none() {
                    problems.push(format!("{label} references missing node '{end}'"));
                }
            }
            if !edge.transition_duration.is_finite() || edge.transition_duration < 0.0 {
                problems.push(format!("{label} has a negative or non-finite duration"));
            }
            for condition in &edge.conditions {
                self.check_condition(&label, condition, problems);
            }
        }
    }
    fn check_condition(&self, label: &str, condition: &Condition, problems: &mut Vec<String>) {
        let name = condition.parameter();
        let Some(declared) = self.parameters.get(name) else {
            problems.push(format!(
                "{label} conditions on undeclared parameter '{name}'"
            ));
            return;
        };
        let matches = matches!(
            (condition, declared),
            (Condition::Bool { .. }, ParameterDeclaration::Bool(_))
                | (Condition::Float { .. }, ParameterDeclaration::Float(_))
                | (Condition::Int { .. }, ParameterDeclaration::Int(_))
                | (Condition::Trigger { .. }, ParameterDeclaration::Trigger)
        );
        if !matches {
            problems.push(format!(
                "{label} conditions on parameter '{name}' with a type-mismatched operator"
            ));
        }
    }
}
