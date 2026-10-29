//! src/procgen/ops/field.rs — a socketable param resolved for evaluation (#1494).
//!
//! The runner hands an op its params' links as images ([`Links`]); each
//! [`Param`] then resolves to a [`Field`]: one value for the whole map, or the
//! linked node's red channel stretched to `[min, max]`. An op reads it per pixel
//! with [`Field::at`] (or [`Field::at_pixel`] from a generator's `(u, v)`), or at a
//! point between pixels with [`Field::at_uv`]; its change from a plain number is
//! mostly `strength` → `strength.at(i)`. A number resolves to [`Field::Const`],
//! whose every read is that number, so an all-number recipe bakes as before.

use super::super::image_buf::Image;
use super::super::recipe::Param;

/// A param's value across the map.
#[derive(Clone, Copy, Debug)]
pub enum Field<'a> {
    /// One value everywhere.
    Const(f32),
    /// `min + (max − min) × img.red` at each pixel.
    Map { img: &'a Image, min: f32, max: f32 },
}

impl Field<'_> {
    /// The value at pixel `index` (row-major) of an image the size of the recipe.
    #[inline]
    pub fn at(&self, index: usize) -> f32 {
        match *self {
            Field::Const(c) => c,
            Field::Map { img, min, max } => stretch(min, max, img.pixels()[index][0]),
        }
    }

    /// The value at the pixel whose centre is `(u, v)`: a generator's sample point.
    #[inline]
    pub fn at_pixel(&self, u: f32, v: f32) -> f32 {
        match *self {
            Field::Const(c) => c,
            Field::Map { img, .. } => {
                let (w, h) = (img.width() as usize, img.height() as usize);
                let x = ((u * w as f32) as usize).min(w - 1);
                let y = ((v * h as f32) as usize).min(h - 1);
                self.at(y * w + x)
            }
        }
    }

    /// The value at a domain point `(u, v)`, bilinear and wrapped, each axis by its
    /// own pixel count, as [`Image::sample_bilinear_wrapped`] reads.
    #[inline]
    pub fn at_uv(&self, u: f32, v: f32) -> f32 {
        match *self {
            Field::Const(c) => c,
            Field::Map { img, min, max } => stretch(min, max, img.sample_bilinear_wrapped(u, v)[0]),
        }
    }

    /// The one value the field takes, if it takes one: a number, or a link whose
    /// red is flat. An op whose varying path differs from its fixed one (a blur)
    /// takes the fixed path then, so a flat link bakes exactly like its number.
    pub fn constant(&self) -> Option<f32> {
        match *self {
            Field::Const(v) => Some(v),
            Field::Map { img, .. } => {
                let first = img.pixels().first()?[0];
                let flat = img
                    .pixels()
                    .iter()
                    .all(|p| p[0].to_bits() == first.to_bits());
                flat.then(|| self.at(0))
            }
        }
    }

    /// The largest value the field takes (NaN pixels ignored); every [`Self::at_uv`]
    /// read lies between two of its values.
    pub fn max(&self) -> f32 {
        match *self {
            Field::Const(c) => c,
            Field::Map { img, .. } => (0..img.pixels().len())
                .map(|i| self.at(i))
                .fold(f32::NEG_INFINITY, f32::max),
        }
    }
}

/// `min + (max − min) × red`: a link to `constant` v over `[0, 1]` reads exactly v.
#[inline]
fn stretch(min: f32, max: f32, red: f32) -> f32 {
    min + (max - min) * red
}

/// The images a node's links name, by node id: what its params resolve against.
#[derive(Default)]
pub struct Links<'a> {
    nodes: Vec<(&'a str, &'a Image)>,
}

impl<'a> Links<'a> {
    /// The linked images, each with its node's id.
    pub fn new(nodes: Vec<(&'a str, &'a Image)>) -> Self {
        Self { nodes }
    }

    /// `param` as a [`Field`]. Panics on a link to a node not handed in: the runner
    /// wires every link a node's params name before it evaluates the node.
    pub fn field(&self, param: &Param) -> Field<'a> {
        match param {
            Param::Num(v) => Field::Const(*v),
            Param::Link(link) => {
                let img = self
                    .nodes
                    .iter()
                    .find(|(id, _)| *id == link.node)
                    .map(|&(_, img)| img)
                    .expect("the runner wires every link");
                Field::Map {
                    img,
                    min: link.min,
                    max: link.max,
                }
            }
        }
    }
}
