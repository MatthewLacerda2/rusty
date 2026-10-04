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

use crate::core::collections::Map;

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
    let target = match &recipe.output {
        Some(id) => id.as_str(),
        None => recipe.nodes.last().map_or("", |n| n.id.as_str()),
    };
    Ok(evaluate_many(recipe, &[target])?.remove(0))
}

/// Evaluate several output nodes of `recipe` in one pass (#403), returning their
/// images in the order of `targets`. Every node runs at most once, so upstream nodes
/// the outputs share are evaluated once — and each image is byte-identical to the
/// one [`evaluate`] gives with that node as the single `output`.
pub fn evaluate_many(recipe: &TextureRecipe, targets: &[&str]) -> Result<Vec<Image>, RunError> {
    if recipe.resolution == 0 {
        return Err(RunError::BadResolution);
    }
    if recipe.nodes.is_empty() {
        return Err(RunError::Empty);
    }

    let by_id: Map<&str, usize> = recipe
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();

    let indices = targets
        .iter()
        .map(|id| {
            by_id
                .get(id)
                .copied()
                .ok_or_else(|| RunError::UnknownOutput(id.to_string()))
        })
        .collect::<Result<Vec<usize>, _>>()?;

    let mut cache: Map<usize, Image> = Map::default();
    let mut state = vec![Visit::Unseen; recipe.nodes.len()];
    for &i in &indices {
        eval_index(recipe, &by_id, i, &mut cache, &mut state)?;
    }
    Ok(indices
        .iter()
        .map(|i| cache.get(i).expect("output evaluated").clone())
        .collect())
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
    by_id: &Map<&str, usize>,
    idx: usize,
    cache: &mut Map<usize, Image>,
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
        let recipe = TextureRecipe::new(
            8,
            vec![
                node(
                    "c",
                    OpKind::Constant {
                        color: [0.2, 0.4, 0.6, 1.0],
                    },
                    &[],
                ),
                node("inv", OpKind::Invert, &["c"]),
            ],
        )
        .with_output("inv");
        let img = evaluate(&recipe).unwrap();
        let p = img.pixels()[0];
        assert!((p[0] - 0.8).abs() < 1e-5);
        assert!((p[1] - 0.6).abs() < 1e-5);
        assert!((p[2] - 0.4).abs() < 1e-5);
    }

    #[test]
    fn missing_input_is_reported() {
        let recipe = TextureRecipe::new(8, vec![node("inv", OpKind::Invert, &["ghost"])]);
        assert!(matches!(
            evaluate(&recipe),
            Err(RunError::UnknownInput { .. })
        ));
    }

    #[test]
    fn cycle_is_detected() {
        let recipe = TextureRecipe::new(
            8,
            vec![
                node("a", OpKind::Invert, &["b"]),
                node("b", OpKind::Invert, &["a"]),
            ],
        )
        .with_output("a");
        assert!(matches!(evaluate(&recipe), Err(RunError::Cycle(_))));
    }

    #[test]
    fn empty_and_bad_resolution_error() {
        let empty = TextureRecipe::new(8, vec![]);
        assert_eq!(evaluate(&empty), Err(RunError::Empty));
        let bad = TextureRecipe::new(0, vec![node("c", OpKind::WhiteNoise, &[])]);
        assert_eq!(evaluate(&bad), Err(RunError::BadResolution));
    }
}
