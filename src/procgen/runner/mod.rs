//! src/procgen/runner — the op-runner: evaluate a recipe DAG to its output images.
//!
//! Two steps. `plan` checks the whole recipe before anything runs (resolution,
//! unique ids, every node's input [`Arity`](super::recipe::Arity), every id it
//! names, no cycle anywhere) and orders the nodes the targets need, counting each
//! one's consumers. A param's link to another node (#1494) is an edge like an
//! input. Then the
//! plan runs: each node's op through [`super::ops::eval_linked`] with its inputs'
//! and links' buffers, and an image is dropped the moment its last consumer has run, so a long
//! chain holds a couple of images at once, not one per node (#1485). Working buffers
//! are linear `[f32; 4]` RGBA and the sampling domain wraps, so results tile by
//! construction.
//!
//! [`evaluate_each`] runs every node instead and hands each image to a hook the
//! moment it is made, before it can be dropped: [`stats`] and [`preview`] read a
//! whole recipe that way without keeping its images alive (#1486).
//!
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

/// Why a recipe could not be evaluated.
#[derive(Debug, PartialEq)]
pub enum RunError {
    /// The recipe has no nodes to evaluate.
    Empty,
    /// Two nodes share an id.
    DuplicateId(String),
    /// The recipe has neither a `resolution` above 0 nor a `size`.
    BadResolution,
    /// One node is wrong (its arity, an input or link id no node defines, a cycle
    /// through it, its image file missing), named by its id and op (#1575).
    Node(NodeError),
    /// The named output node is not in the recipe.
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

/// Evaluate `recipe` to its output [`Image`]: [`evaluate_with`] and no image files,
/// for a recipe without `image` nodes.
pub fn evaluate(recipe: &TextureRecipe) -> Result<Image, RunError> {
    evaluate_with(recipe, &Sources::new())
}

/// Evaluate `recipe` to its output [`Image`], its `image` nodes reading `sources`.
/// The output is the explicit `output` node if set, else the last node in the list.
pub fn evaluate_with(recipe: &TextureRecipe, sources: &Sources) -> Result<Image, RunError> {
    let target = match &recipe.output {
        Some(id) => id.as_str(),
        None => recipe.nodes.last().map_or("", |n| n.id.as_str()),
    };
    Ok(evaluate_many(recipe, sources, &[target])?.remove(0))
}

/// Evaluate several output nodes of `recipe` in one pass (#403), returning their
/// images in the order of `targets`. Every node runs at most once, so upstream nodes
/// the outputs share are evaluated once — and each image is byte-identical to the
/// one [`evaluate`] gives with that node as the single `output`.
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

/// Run every structural check on `recipe` (resolution, ids, arity, inputs, the
/// declared outputs, cycles) without evaluating it. A caller runs it before it
/// reads the recipe's image files, so a broken recipe is reported as itself.
pub fn validate(recipe: &TextureRecipe) -> Result<(), RunError> {
    plan(recipe, &[], Scope::Every).map(|_| ())
}

/// Refuse a recipe with an `image` node whose file `sources` lacks, before anything
/// evaluates: a missing file is an error naming it, never a black canvas (#1485).
fn check_sources(recipe: &TextureRecipe, sources: &Sources) -> Result<(), RunError> {
    match recipe
        .image_paths()
        .into_iter()
        .find(|&(_, path)| !sources.has(path))
    {
        Some((node, path)) => Err(RunError::Node(NodeError {
            node: node.into(),
            op: "image".into(),
            fault: Fault::MissingSource(path.into()),
        })),
        None => Ok(()),
    }
}

/// A finished run: the target images, and the most images and bytes it held at once.
struct Run {
    images: Vec<Image>,
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "read by the O(1)-live-images test (#1485)")
    )]
    peak_live: usize,
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "read by the gray-chain peak test (#2002)")
    )]
    peak_bytes: usize,
}

/// Evaluate `plan`'s nodes in order, handing each image to `on_node` as it is made
/// and dropping it once its last consumer has run, unless it is a target. A waiting
/// image is held as one channel when it is grayscale, and a lazily loaded file only
/// while an `image` node still to run reads it ([`held`], #2002); a file the loader
/// can't read is the one error a run can still meet. A node reads each distinct
/// input once, and an input it is the last reader of is moved out of its slot, so a
/// gray chain peaks at an input and its output, as an RGBA one does.
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
        // Drop each image this node was the last reader of (the ones it did not
        // take already). A node nothing reads (a sink under `Scope::Every`) goes as
        // soon as the hook has seen it; a target is kept whole.
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

/// Run node `i`'s op on its gathered inputs, returning its output and the images
/// and bytes its inputs owned while it ran (all freed before it returns).
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

/// The targets' images in request order, each rebuilt whole; one named twice is
/// copied for all but its last request.
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
