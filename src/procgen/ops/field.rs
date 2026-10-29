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
    /// Flat means every stretched value is the same: a link with `min == max` is
    /// flat whatever it links, and `0.0` and `-0.0` count as one value.
    pub fn constant(&self) -> Option<f32> {
        match *self {
            Field::Const(v) => Some(v),
            Field::Map { img, .. } => {
                let first = self.at(0);
                let same = |v: f32| v == first || v.to_bits() == first.to_bits();
                (0..img.pixels().len())
                    .all(|i| same(self.at(i)))
                    .then_some(first)
            }
        }
    }

    /// Whether the field is a link (its value may vary across the map).
    pub fn linked(&self) -> bool {
        matches!(self, Field::Map { .. })
    }

    /// How many pixels the linked image holds; 0 for a number.
    fn pixels(&self) -> usize {
        match *self {
            Field::Const(_) => 0,
            Field::Map { img, .. } => img.pixels().len(),
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

/// A value per cell of a wrapped `per`×`per` grid, worked out once per cell rather
/// than once per pixel that looks at the cell (#1494): `tile`'s jitters and
/// Voronoi's randomness read their links at each cell's centre. A grid finer than
/// its linked images has no table, since its cells outnumber the pixels that would
/// read them; with no link there is nothing to save, so each read is the closure.
pub struct PerCell<T, F> {
    per: i64,
    table: Option<Vec<T>>,
    value: F,
}

impl<T: Copy, F: Fn((i64, i64)) -> T> PerCell<T, F> {
    /// The grid's `value` per (wrapped) cell, tabled when one of `fields` links.
    pub fn new(per: i64, fields: &[Field], value: F) -> Self {
        let pixels = fields.iter().map(Field::pixels).max().unwrap_or(0);
        let fits = per.checked_mul(per).is_some_and(|c| c as usize <= pixels);
        let tabled = fields.iter().any(Field::linked) && fits;
        let table = tabled.then(|| (0..per * per).map(|i| value((i % per, i / per))).collect());
        Self { per, table, value }
    }

    /// The value of the wrapped cell `cell` (each coordinate in `0..per`).
    #[inline]
    pub fn get(&self, cell: (i64, i64)) -> T {
        match &self.table {
            Some(t) => t[(cell.1 * self.per + cell.0) as usize],
            None => (self.value)(cell),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_per_cell_table_holds_what_each_cell_reads() {
        let mask = Image::fill_uv(8, |u, v| [u * v, 0.0, 0.0, 1.0]);
        let link = [Field::Map {
            img: &mask,
            min: 0.0,
            max: 1.0,
        }];
        let read = |c: (i64, i64)| link[0].at_uv(c.0 as f32 / 4.0, c.1 as f32 / 4.0);
        let cells = PerCell::new(4, &link, read);
        assert!(cells.table.is_some(), "16 cells fit 64 pixels");
        (0..16).for_each(|i| assert_eq!(cells.get((i % 4, i / 4)), read((i % 4, i / 4))));
        assert!(
            PerCell::new(9, &link, read).table.is_none(),
            "81 cells don't"
        );
    }
}
