//! crates/procgen/src/ops/mod.rs — the curated op-set's math, grouped by category, plus
//! the single-node entry point the runner calls.
//!
//! The op-set is "Blender-lite": the actually-used subset of Blender's texture/shader
//! nodes — enough to author the maps a PBR material needs, not a Substance clone. Ops
//! are grouped like Blender's node menus:
//!
//! - [`generators`] — sources with no input (constant, noise and its fBM / ridged /
//!   turbulence family, Voronoi F1 / F2 / edges, gradient,
//!   wave, brick, shape, checker, white noise, the pixel's own coordinates, and an
//!   image file).
//! - [`color`] — mix (blend modes), invert, bright/contrast, HSV, gamma, auto
//!   levels; [`ramp`] — the color ramp.
//! - [`vector`] — domain mapping, domain warp, bump→normal, normal blend,
//!   combine/separate RGB; [`sample`] — sampling a map at given coordinates;
//!   [`tile`] — the seeded grid scatter.
//! - [`math`] — math (against a constant or a second image), map range, clamp,
//!   RGB→BW.
//! - [`filter`] — neighbourhood ops: separable box or Gaussian blur, and the
//!   height-derived masks cavity and curvature; [`distance`] — a mask's distance
//!   from its edge, a bevel for any shape; [`flood`] — a mask's islands, each with
//!   its centroid and box, for the flood-fill ops.
//!
//! These are the ops' math. Each op itself (its params, catalog row and how it
//! calls this math) is one file in [`super::catalog`] (#1751), and [`eval_node`]
//! runs one through [`OpKind::run`], so the Lua surface, a `.json` recipe, and a
//! future material/shader leg all route through identical math. A socketable param
//! reaches its op as a [`field::Field`], a number or a linked node's map (#1494).

// sorted: one module per family's math; branches adding math land on different lines
pub mod color;
pub mod distance;
#[cfg(test)]
pub mod dummy;
pub mod field;
pub mod filter;
pub mod flood;
pub mod generators;
pub mod math;
pub mod ramp;
pub mod sample;
pub mod tile;
pub mod vector;

use super::catalog::Cx;
use super::image_buf::{Image, Size};
use super::recipe::OpKind;
use super::source::Sources;
use field::Links;

/// Evaluate one node's op with no links: [`eval_linked`] for a node whose params
/// are all numbers.
pub fn eval_node(
    op: &OpKind,
    inputs: &[&Image],
    size: Size,
    seed: u64,
    sources: &Sources,
) -> Image {
    eval_linked(op, inputs, &Links::default(), size, seed, sources)
}

/// Evaluate one node's op. `inputs` are the buffers of the node's input ids in order;
/// `links` the images its params link to (#1494); `size` is the recipe's extent, and
/// `seed` is the recipe seed with the node's own folded in
/// ([`super::hash::node_seed`], done by the runner). `sources` holds the decoded
/// files `image` nodes read. Returns the node's output image.
///
/// `inputs` must hold as many images as the op's [`Arity`](super::recipe::Arity)
/// accepts: the runner refuses a miswired node before anything evaluates (#1485), so
/// every required input is read directly, and this panics if called with fewer. The
/// runner likewise refuses an `image` node whose file is not in `sources`, and hands
/// in every node a param links to.
pub fn eval_linked(
    op: &OpKind,
    inputs: &[&Image],
    links: &Links,
    size: Size,
    seed: u64,
    sources: &Sources,
) -> Image {
    op.run(&Cx {
        inputs,
        links,
        size,
        seed,
        sources,
    })
}

#[cfg(test)]
mod arity_tests;
#[cfg(test)]
mod aspect_tests;
#[cfg(test)]
mod link_tests;
#[cfg(test)]
mod seed_tests;
#[cfg(test)]
pub(crate) mod test_graph;
