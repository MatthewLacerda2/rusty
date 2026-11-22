use super::super::image_buf::{Image, Rgba, Size};
use super::super::ops::field::{Field, Links};
use super::super::ops::filter;
use super::super::recipe::{Arity, OpInfo, Param};
use super::super::source::Sources;

/// The tag must be the one serde writes for the op's [`super::OpKind`] variant (its snake_case
/// name); `recipe`'s catalog tests hold the two to the parser.
pub trait Row: Op {
    const INFO: OpInfo;
}

pub trait Op {
    /// `cx` holds as many inputs as the op's arity accepts: the runner refuses a miswired
    /// node first, so a required input is read directly.
    fn run(&self, cx: &Cx) -> Image;

    /// An op with a `seed` param returns it (`every_seed_param_reaches_the_runner` fails otherwise).
    fn seed(&self) -> u32 {
        0
    }

    fn sockets(&self) -> Vec<(&'static str, &Param)> {
        Vec::new()
    }

    /// Inputs accepted when a param narrows the row's arity; `None` keeps the row's.
    fn arity(&self) -> Option<Arity> {
        None
    }

    fn func(&self) -> Option<String> {
        None
    }

    fn arity_fix(&self, _given: usize) -> Option<String> {
        None
    }
}

pub struct Cx<'a> {
    pub inputs: &'a [&'a Image],
    pub links: &'a Links<'a>,
    pub size: Size,
    pub seed: u64,
    pub sources: &'a Sources,
}

impl<'a> Cx<'a> {
    pub fn paint(&self, sampler: impl Fn(f32, f32) -> Rgba + Sync) -> Image {
        Image::fill_uv(self.size, sampler)
    }

    pub fn own(&self) -> Image {
        self.inputs[0].clone()
    }

    pub fn input(&self, i: usize) -> &'a Image {
        self.inputs[i]
    }

    pub fn optional(&self, i: usize) -> Option<&'a Image> {
        self.inputs.get(i).copied()
    }

    pub fn field(&self, p: &Param) -> Field<'a> {
        self.links.field(p)
    }

    /// A radius in tile units as pixels across and down.
    pub fn px(&self, radius: f32) -> (f32, f32) {
        filter::pixel_radii(radius, self.size)
    }
}
