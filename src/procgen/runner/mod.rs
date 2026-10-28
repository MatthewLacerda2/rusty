//! src/procgen/runner — the op-runner: evaluate a recipe DAG to its output images.
//!
//! Two steps. [`plan`] checks the whole recipe before anything runs (resolution,
//! every node's input [`Arity`](super::recipe::Arity), every id it names, no cycle)
//! and orders the nodes the targets need, counting each one's consumers. Then the
//! plan runs: each node's op through [`super::ops::eval_node`] with its inputs'
//! buffers, and an image is dropped the moment its last consumer has run, so a long
//! chain holds a couple of images at once, not one per node (#1485). Working buffers
//! are linear `[f32; 4]` RGBA and the sampling domain wraps, so results tile by
//! construction.
//!
//! Evaluation is a pure function of `(recipe, seed)`: no wall-clock, no global RNG
//! (stochastic ops draw from [`super::hash`]), so the same recipe always yields the
//! same buffer — the foundation of the byte-identical-bake guarantee.

mod plan;

use super::image_buf::Image;
use super::ops::eval_node;
use super::recipe::{ArityError, TextureRecipe};
use plan::{plan, Plan};

/// Why a recipe could not be evaluated.
#[derive(Debug, PartialEq)]
pub enum RunError {
    /// The recipe has no nodes to evaluate.
    Empty,
    /// `resolution` was 0 (or otherwise unusable).
    BadResolution,
    /// A node is wired with fewer or more inputs than its op accepts.
    Arity(ArityError),
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
            RunError::Arity(e) => e.fmt(f),
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
    let plan = plan(recipe, targets)?;
    Ok(run(recipe, &plan).images)
}

/// A finished run: the target images, and the most images it held at once.
struct Run {
    images: Vec<Image>,
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "read by the O(1)-live-images test (#1485)")
    )]
    peak_live: usize,
}

/// Evaluate `plan`'s nodes in order, dropping each image once its last consumer has
/// run unless it is a target.
fn run(recipe: &TextureRecipe, plan: &Plan) -> Run {
    let mut slots: Vec<Option<Image>> = recipe.nodes.iter().map(|_| None).collect();
    let mut uses = plan.uses.clone();
    let (mut live, mut peak_live) = (0, 0);
    for &i in &plan.order {
        let inputs: Vec<&Image> = plan.inputs[i]
            .iter()
            .map(|&j| slots[j].as_ref().expect("plan orders inputs first"))
            .collect();
        let out = eval_node(&recipe.nodes[i].op, &inputs, recipe.resolution, recipe.seed);
        slots[i] = Some(out);
        live += 1;
        peak_live = peak_live.max(live);
        for &j in &plan.inputs[i] {
            uses[j] -= 1;
            if uses[j] == 0 && !plan.targets.contains(&j) {
                slots[j] = None;
                live -= 1;
            }
        }
    }
    // A target named twice is cloned for all but its last request.
    let images = plan
        .targets
        .iter()
        .enumerate()
        .map(|(k, &i)| {
            let image = if plan.targets[k + 1..].contains(&i) {
                slots[i].clone()
            } else {
                slots[i].take()
            };
            image.expect("every target was evaluated")
        })
        .collect();
    Run { images, peak_live }
}

#[cfg(test)]
mod tests;
