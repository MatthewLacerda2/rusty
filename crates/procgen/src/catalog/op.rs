//! crates/procgen/src/catalog/op.rs — what one op declares (#1751): its catalog [`Row`]
//! and its [`Op`] behaviour, both implemented on its params struct, in its own
//! file.

use super::super::image_buf::{Image, Rgba, Size};
use super::super::ops::field::{Field, Links};
use super::super::ops::filter;
use super::super::recipe::{Arity, OpInfo, Param};
use super::super::source::Sources;

/// An op's catalog row: its `op` tag, menu category, the inputs it accepts and its
/// params, as `Texture.Ops()` lists them. The tag must be the one serde writes for
/// the op's [`super::OpKind`] variant (its name in snake_case); `recipe`'s catalog
/// tests hold the two, and every param, to the parser.
pub trait Row: Op {
    const INFO: OpInfo;
}

/// How an op runs. Every method but [`Op::run`] has a default an op overrides only
/// when it needs to.
pub trait Op {
    /// The node's output image. `cx` holds as many inputs as the op's arity
    /// accepts: the runner refuses a miswired node before anything evaluates, so a
    /// required input is read directly.
    fn run(&self, cx: &Cx) -> Image;

    /// The node's own `seed` param, folded into the recipe seed by the runner; 0
    /// for an op that draws no randomness. An op with a `seed` param returns it
    /// (`every_seed_param_reaches_the_runner` fails otherwise).
    fn seed(&self) -> u32 {
        0
    }

    /// The params its row marks `socket`, by name, which a link may drive (#1494).
    fn sockets(&self) -> Vec<(&'static str, &Param)> {
        Vec::new()
    }

    /// The inputs this node accepts when a param narrows its row's arity (a unary
    /// `math` func takes one); `None` keeps the row's.
    fn arity(&self) -> Option<Arity> {
        None
    }

    /// The sub-function an arity error names (`math`'s `func`).
    fn func(&self) -> Option<String> {
        None
    }
}

/// What a node runs with: its inputs (already evaluated, in the node's order), the
/// images its params link to, the recipe's extent, and its seed (the recipe seed
/// with the node's own folded in).
pub struct Cx<'a> {
    pub inputs: &'a [&'a Image],
    pub links: &'a Links<'a>,
    pub size: Size,
    pub seed: u64,
    /// The decoded files `image` nodes read; the runner checks each is here.
    pub sources: &'a Sources,
}

impl<'a> Cx<'a> {
    /// A generator's image: `sampler` painted at every pixel centre.
    pub fn paint(&self, sampler: impl Fn(f32, f32) -> Rgba + Sync) -> Image {
        Image::fill_uv(self.size, sampler)
    }

    /// A copy of the first input, for an op that edits its pixels in place.
    pub fn own(&self) -> Image {
        self.inputs[0].clone()
    }

    /// Input `i`, which the op's arity requires.
    pub fn input(&self, i: usize) -> &'a Image {
        self.inputs[i]
    }

    /// Input `i` when it is wired: a trailing optional input (a mask).
    pub fn optional(&self, i: usize) -> Option<&'a Image> {
        self.inputs.get(i).copied()
    }

    /// A socketable param's value across the map: its number, or its link.
    pub fn field(&self, p: &Param) -> Field<'a> {
        self.links.field(p)
    }

    /// A radius in tile units as pixels across and down.
    pub fn px(&self, radius: f32) -> (f32, f32) {
        filter::pixel_radii(radius, self.size)
    }
}
