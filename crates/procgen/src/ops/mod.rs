// sorted: one module per family's math; branches adding math land on different lines
pub mod color;
pub mod distance;
#[cfg(test)]
pub mod dummy;
pub mod expression;
pub mod field;
pub mod filter;
pub mod flood;
pub mod generators;
pub mod math;
pub mod outline;
pub mod ramp;
pub mod sample;
pub mod scatter;
pub mod tile;
pub mod vector;

use super::catalog::Cx;
use super::image_buf::{Image, Size};
use super::recipe::OpKind;
use super::source::Sources;
use field::Links;

/// A test hook: the crate's tests run one node with no links.
pub fn eval_node(
    op: &OpKind,
    inputs: &[&Image],
    size: Size,
    seed: u64,
    sources: &Sources,
) -> Image {
    eval_linked(op, inputs, &Links::default(), size, seed, sources)
}

/// `inputs` must hold as many images as the op's [`Arity`](super::recipe::Arity)
/// accepts: the runner refuses a miswired node before anything evaluates, so
/// every required input is read directly, and this panics if called with fewer. The
/// runner likewise refuses an `image` node whose file is not in `sources`, and hands
/// in every node a param links to, and passes `seed` with the node's own folded in.
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
