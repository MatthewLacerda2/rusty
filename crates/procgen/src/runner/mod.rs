//! Evaluation is a pure function of `(recipe, seed, sources)`: no wall-clock, no global RNG
//! (stochastic ops draw from [`super::hash`]), so the same recipe always yields the
//! same buffer — the foundation of the byte-identical-bake guarantee.

mod held;
mod plan;
pub mod preview;
mod slots;
pub mod stats;

use super::hash::node_seed;
use super::image_buf::Image;
use super::ops::eval_linked;
use super::ops::field::Links;
use super::recipe::{Fault, NodeError, TextureRecipe};
use super::source::Sources;
use held::{Held, Resident};
use plan::{plan, Plan, Scope};
use slots::{image_bytes, Slots};

#[derive(Debug, PartialEq)]
pub enum RunError {
    Empty,
    DuplicateId(String),
    BadResolution,
    Node(NodeError),
    UnknownOutput(String),
}

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunError::Empty => write!(f, "recipe has no nodes"),
            RunError::DuplicateId(id) => write!(f, "two nodes share the id '{id}'"),
            RunError::BadResolution => write!(f, "recipe needs a resolution > 0 or a size"),
            RunError::Node(e) => e.fmt(f),
            RunError::UnknownOutput(id) => write!(f, "output node '{id}' not found"),
        }
    }
}

pub fn evaluate(recipe: &TextureRecipe) -> Result<Image, RunError> {
    evaluate_with(recipe, &Sources::new())
}

pub fn evaluate_with(recipe: &TextureRecipe, sources: &Sources) -> Result<Image, RunError> {
    let target = match &recipe.output {
        Some(id) => id.as_str(),
        None => recipe.nodes.last().map_or("", |n| n.id.as_str()),
    };
    Ok(evaluate_many(recipe, sources, &[target])?.remove(0))
}

pub fn evaluate_many(
    recipe: &TextureRecipe,
    sources: &Sources,
    targets: &[&str],
) -> Result<Vec<Image>, RunError> {
    let plan = plan(recipe, targets, Scope::Needed)?;
    check_sources(recipe, sources)?;
    Ok(run(recipe, sources, &plan, &mut |_, _| {})?.images)
}

/// Evaluate every node of `recipe`, calling `on_node(index, image)` as each one
/// finishes (`index` into `recipe.nodes`). No image outlives its last consumer, so
/// the hook takes what it needs then and there: a recipe's whole set of images is
/// never held at once.
pub fn evaluate_each(
    recipe: &TextureRecipe,
    sources: &Sources,
    mut on_node: impl FnMut(usize, &Image),
) -> Result<(), RunError> {
    let plan = plan(recipe, &[], Scope::Every)?;
    check_sources(recipe, sources)?;
    run(recipe, sources, &plan, &mut on_node)?;
    Ok(())
}

pub fn validate(recipe: &TextureRecipe) -> Result<(), RunError> {
    plan(recipe, &[], Scope::Every).map(|_| ())
}

fn check_sources(recipe: &TextureRecipe, sources: &Sources) -> Result<(), RunError> {
    let missing = |node: &str, op: &str, fault: Fault| {
        Err(RunError::Node(NodeError {
            node: node.into(),
            op: op.into(),
            fault,
        }))
    };
    if let Some((node, path)) = recipe
        .image_paths()
        .into_iter()
        .find(|&(_, path)| !sources.has(path))
    {
        return missing(node, "image", Fault::MissingSource(path.into()));
    }
    match recipe
        .font_paths()
        .into_iter()
        .find(|&(_, path)| sources.font(path).is_none())
    {
        Some((node, path)) => missing(node, "text", Fault::MissingFont(path.into())),
        None => Ok(()),
    }
}

struct Run {
    images: Vec<Image>,
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "read by the O(1)-live-images test")
    )]
    peak_live: usize,
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "read by the gray-chain peak test")
    )]
    peak_bytes: usize,
}

fn run(
    recipe: &TextureRecipe,
    sources: &Sources,
    plan: &Plan,
    on_node: &mut dyn FnMut(usize, &Image),
) -> Result<Run, RunError> {
    let mut slots = Slots::new(recipe.nodes.len());
    let mut resident = Resident::new(recipe, &plan.order, sources);
    let mut uses = plan.uses.clone();
    for &i in &plan.order {
        let files = resident
            .for_node(recipe, i, sources)
            .map_err(RunError::Node)?;
        let (out, (images, bytes)) = eval_one(recipe, plan, i, &mut slots, &uses, files);
        slots.note(images + 1, bytes + image_bytes(&out));
        resident.done(recipe, i);
        on_node(i, &out);
        for &j in &plan.inputs[i] {
            uses[j] -= 1;
        }
        for &j in &plan.inputs[i] {
            if uses[j] == 0 && !plan.targets.contains(&j) {
                slots.take(j);
            }
        }
        if plan.targets.contains(&i) {
            slots.put(i, Held::Rgba(out));
        } else if uses[i] > 0 {
            let out_bytes = image_bytes(&out);
            let held = Held::pack(out);
            if matches!(held, Held::Gray(..)) {
                slots.note(1, out_bytes + held.bytes());
            }
            slots.put(i, held);
        }
    }
    let (peak_live, peak_bytes) = (slots.peak_live, slots.peak_bytes);
    let images = take_targets(&plan.targets, &mut slots.into_inner());
    Ok(Run {
        images,
        peak_live,
        peak_bytes,
    })
}

fn eval_one(
    recipe: &TextureRecipe,
    plan: &Plan,
    i: usize,
    slots: &mut Slots,
    uses: &[usize],
    files: &Sources,
) -> (Image, (usize, usize)) {
    let node = &recipe.nodes[i];
    let gathered = slots.gather(&plan.inputs[i], uses, &plan.targets);
    let edges = gathered.edges();
    let (inputs, linked) = edges.split_at(node.inputs.len());
    let ids = plan.inputs[i][node.inputs.len()..].iter();
    let links = Links::new(
        ids.map(|&j| recipe.nodes[j].id.as_str())
            .zip(linked.iter().copied())
            .collect(),
    );
    let seed = node_seed(recipe.seed, node.op.seed());
    let out = eval_linked(&node.op, inputs, &links, recipe.extent(), seed, files);
    (out, gathered.owned())
}

fn take_targets(targets: &[usize], slots: &mut [Option<Held>]) -> Vec<Image> {
    let take = |k: usize, i: usize, slots: &mut [Option<Held>]| {
        let held = if targets[k + 1..].contains(&i) {
            slots[i].as_ref().map(|h| h.view().into_owned())
        } else {
            slots[i].take().map(Held::into_image)
        };
        held.expect("every target was evaluated")
    };
    let mut images = Vec::with_capacity(targets.len());
    for (k, &i) in targets.iter().enumerate() {
        images.push(take(k, i, slots));
    }
    images
}

#[cfg(test)]
mod each_tests;
#[cfg(test)]
mod held_tests;
#[cfg(test)]
mod slots_tests;
#[cfg(test)]
mod tests;
