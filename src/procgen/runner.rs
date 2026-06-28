//! src/procgen/runner.rs — the op-runner: evaluate a recipe DAG to one image.
//!
//! Walks the [`TextureRecipe`] in dependency order (a depth-first topological sort),
//! evaluating each node's op via [`super::ops::eval_node`] with its inputs' buffers,
//! and returns the output node's [`Image`]. Working buffers are linear `[f32; 4]`
//! RGBA and the sampling domain wraps, so the result tiles by construction.
//!
//! Evaluation is a pure function of `(recipe, seed)`: no wall-clock, no global RNG
//! (stochastic ops draw from [`super::hash`]), so the same recipe always yields the
//! same buffer — the foundation of the byte-identical-bake guarantee.

use std::collections::HashMap;

use super::image_buf::Image;
use super::ops::eval_node;
use super::recipe::TextureRecipe;

/// Why a recipe could not be evaluated.
#[derive(Debug, PartialEq)]
pub enum RunError {
    /// The recipe has no nodes to evaluate.
    Empty,
    /// `resolution` was 0 (or otherwise unusable).
    BadResolution,
    /// A node referenced an input id that no node defines.
    UnknownInput { node: String, input: String },
    /// The named output node is not in the recipe.
    UnknownOutput(String),
    /// The graph contains a cycle (not a DAG).
    Cycle(String),
}

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunError::Empty => write!(f, "recipe has no nodes"),
            RunError::BadResolution => write!(f, "recipe resolution must be > 0"),
            RunError::UnknownInput { node, input } => {
                write!(f, "node '{node}' references unknown input '{input}'")
            }
            RunError::UnknownOutput(id) => write!(f, "output node '{id}' not found"),
            RunError::Cycle(id) => write!(f, "recipe DAG has a cycle at node '{id}'"),
        }
    }
}

/// Evaluate `recipe` to its output [`Image`]. The output is the explicit `output`
/// node if set, else the last node in the list.
pub fn evaluate(recipe: &TextureRecipe) -> Result<Image, RunError> {
    if recipe.resolution == 0 {
        return Err(RunError::BadResolution);
    }
    if recipe.nodes.is_empty() {
        return Err(RunError::Empty);
    }

    let by_id: HashMap<&str, usize> = recipe
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();

    let target = match &recipe.output {
        Some(id) => *by_id
            .get(id.as_str())
            .ok_or_else(|| RunError::UnknownOutput(id.clone()))?,
        None => recipe.nodes.len() - 1,
    };

    let mut cache: HashMap<usize, Image> = HashMap::new();
    let mut state = vec![Visit::Unseen; recipe.nodes.len()];
    eval_index(recipe, &by_id, target, &mut cache, &mut state)?;
    Ok(cache.remove(&target).expect("output evaluated"))
}

/// DFS visit state for cycle detection.
#[derive(Clone, Copy, PartialEq)]
enum Visit {
    Unseen,
    OnStack,
    Done,
}

/// Recursively evaluate node `idx` (and its inputs first), memoizing into `cache`.
fn eval_index(
    recipe: &TextureRecipe,
    by_id: &HashMap<&str, usize>,
    idx: usize,
    cache: &mut HashMap<usize, Image>,
    state: &mut [Visit],
) -> Result<(), RunError> {
    if state[idx] == Visit::Done {
        return Ok(());
    }
    if state[idx] == Visit::OnStack {
        return Err(RunError::Cycle(recipe.nodes[idx].id.clone()));
    }
    state[idx] = Visit::OnStack;

    let node = &recipe.nodes[idx];
    let mut input_indices = Vec::with_capacity(node.inputs.len());
    for input in &node.inputs {
        let i = *by_id
            .get(input.as_str())
            .ok_or_else(|| RunError::UnknownInput {
                node: node.id.clone(),
                input: input.clone(),
            })?;
        eval_index(recipe, by_id, i, cache, state)?;
        input_indices.push(i);
    }

    let inputs: Vec<&Image> = input_indices
        .iter()
        .map(|i| cache.get(i).expect("input cached"))
        .collect();
    let out = eval_node(&node.op, &inputs, recipe.resolution, recipe.seed);
    cache.insert(idx, out);
    state[idx] = Visit::Done;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::procgen::recipe::{Node, OpKind};

    fn node(id: &str, op: OpKind, inputs: &[&str]) -> Node {
        Node {
            id: id.into(),
            op,
            inputs: inputs.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn evaluates_in_dependency_order() {
        let recipe = TextureRecipe {
            resolution: 8,
            seed: 0,
            nodes: vec![
                node(
                    "c",
                    OpKind::Constant {
                        color: [0.2, 0.4, 0.6, 1.0],
                    },
                    &[],
                ),
                node("inv", OpKind::Invert, &["c"]),
            ],
            output: Some("inv".into()),
        };
        let img = evaluate(&recipe).unwrap();
        let p = img.pixels()[0];
        assert!((p[0] - 0.8).abs() < 1e-5);
        assert!((p[1] - 0.6).abs() < 1e-5);
        assert!((p[2] - 0.4).abs() < 1e-5);
    }

    #[test]
    fn missing_input_is_reported() {
        let recipe = TextureRecipe {
            resolution: 8,
            seed: 0,
            nodes: vec![node("inv", OpKind::Invert, &["ghost"])],
            output: None,
        };
        assert!(matches!(
            evaluate(&recipe),
            Err(RunError::UnknownInput { .. })
        ));
    }

    #[test]
    fn cycle_is_detected() {
        let recipe = TextureRecipe {
            resolution: 8,
            seed: 0,
            nodes: vec![
                node("a", OpKind::Invert, &["b"]),
                node("b", OpKind::Invert, &["a"]),
            ],
            output: Some("a".into()),
        };
        assert!(matches!(evaluate(&recipe), Err(RunError::Cycle(_))));
    }

    #[test]
    fn empty_and_bad_resolution_error() {
        let empty = TextureRecipe {
            resolution: 8,
            seed: 0,
            nodes: vec![],
            output: None,
        };
        assert_eq!(evaluate(&empty), Err(RunError::Empty));
        let bad = TextureRecipe {
            resolution: 0,
            seed: 0,
            nodes: vec![node("c", OpKind::WhiteNoise, &[])],
            output: None,
        };
        assert_eq!(evaluate(&bad), Err(RunError::BadResolution));
    }
}
