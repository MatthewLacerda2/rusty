//! src/asset/animation_graph/validate.rs — graph validation (#315).
//!
//! The guardrail between the serialized document and everything that consumes it:
//! a graph that loads is a graph whose references resolve — the entry and every
//! edge point at real nodes, and every condition reads a *declared* parameter with
//! a type-compatible operator. Violations are collected (not first-error-wins), so
//! an authoring agent sees the whole fix list in one pass.

use std::collections::BTreeSet;
use std::fmt;

use super::{AnimationGraph, Condition, ParameterDeclaration};

/// Anything that can go wrong loading, saving, or validating a graph. String-y
/// like `ImportError`: the asset layer is a leaf and callers surface these to a
/// log/console.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphError {
    /// Reading/writing the graph file failed.
    Io(String),
    /// The file isn't a well-formed graph document.
    Parse(String),
    /// The document parsed but its references/values don't hold up — every
    /// violation, one message each.
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
    /// Check every internal reference and value: unique named nodes, an entry that
    /// exists, edges between existing nodes with sane durations, and conditions
    /// over declared, type-matching parameters. `Ok(())` or ALL the violations.
    pub fn validate(&self) -> Result<(), GraphError> {
        let mut problems = Vec::new();
        self.check_nodes(&mut problems);
        self.check_entry(&mut problems);
        self.check_edges(&mut problems);
        if problems.is_empty() {
            Ok(())
        } else {
            Err(GraphError::Invalid(problems))
        }
    }

    fn check_nodes(&self, problems: &mut Vec<String>) {
        let mut seen = BTreeSet::new();
        for node in &self.nodes {
            if node.name.is_empty() {
                problems.push("a node has an empty name".to_string());
            }
            if !seen.insert(node.name.as_str()) {
                problems.push(format!("duplicate node name '{}'", node.name));
            }
            if node.clip.is_empty() {
                problems.push(format!("node '{}' has an empty clip name", node.name));
            }
            if node.speed.is_some_and(|s| !s.is_finite()) {
                problems.push(format!("node '{}' has a non-finite speed", node.name));
            }
        }
    }

    fn check_entry(&self, problems: &mut Vec<String>) {
        if self.node(&self.entry).is_none() {
            problems.push(format!("entry node '{}' does not exist", self.entry));
        }
    }

    fn check_edges(&self, problems: &mut Vec<String>) {
        for (i, edge) in self.edges.iter().enumerate() {
            let label = format!("edge #{i} ('{}' -> '{}')", edge.from, edge.to);
            for end in [&edge.from, &edge.to] {
                if self.node(end).is_none() {
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

    /// A condition must read a *declared* parameter, and its variant (which keys
    /// the operator set) must match the declared type.
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
