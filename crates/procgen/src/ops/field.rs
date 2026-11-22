use super::super::image_buf::Image;
use super::super::recipe::Param;

#[derive(Clone, Copy, Debug)]
pub enum Field<'a> {
    Const(f32),
    Map { img: &'a Image, min: f32, max: f32 },
}

impl Field<'_> {
    #[inline]
    pub fn at(&self, index: usize) -> f32 {
        match *self {
            Field::Const(c) => c,
            Field::Map { img, min, max } => stretch(min, max, img.pixels()[index][0]),
        }
    }

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

    #[inline]
    pub fn at_uv(&self, u: f32, v: f32) -> f32 {
        match *self {
            Field::Const(c) => c,
            Field::Map { img, min, max } => stretch(min, max, img.sample_bilinear_wrapped(u, v)[0]),
        }
    }

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

    pub fn linked(&self) -> bool {
        matches!(self, Field::Map { .. })
    }

    fn pixels(&self) -> usize {
        match *self {
            Field::Const(_) => 0,
            Field::Map { img, .. } => img.pixels().len(),
        }
    }

    pub fn max(&self) -> f32 {
        match *self {
            Field::Const(c) => c,
            Field::Map { img, .. } => (0..img.pixels().len())
                .map(|i| self.at(i))
                .fold(f32::NEG_INFINITY, f32::max),
        }
    }
}

pub struct PerCell<T, F> {
    per: (i64, i64),
    table: Option<Vec<T>>,
    value: F,
}

impl<T: Copy, F: Fn((i64, i64)) -> T> PerCell<T, F> {
    pub fn new(per: i64, fields: &[Field], value: F) -> Self {
        Self::grid((per, per), fields, value)
    }

    pub fn grid(per: (i64, i64), fields: &[Field], value: F) -> Self {
        let pixels = fields.iter().map(Field::pixels).max().unwrap_or(0);
        let fits = per
            .0
            .checked_mul(per.1)
            .is_some_and(|c| c as usize <= pixels);
        let tabled = fields.iter().any(Field::linked) && fits;
        let (w, n) = (per.0, per.0 * per.1);
        let table = tabled.then(|| (0..n).map(|i| value((i % w, i / w))).collect());
        Self { per, table, value }
    }

    /// The quarter keeps the table well under the image's own size.
    pub fn for_pixels(per: (i64, i64), pixels: usize, value: F) -> Self {
        let fits = per
            .0
            .checked_mul(per.1)
            .and_then(|c| c.checked_mul(4))
            .is_some_and(|c| c as usize <= pixels);
        let (w, n) = (per.0, per.0 * per.1);
        let table = fits.then(|| (0..n).map(|i| value((i % w, i / w))).collect());
        Self { per, table, value }
    }

    #[inline]
    pub fn get(&self, cell: (i64, i64)) -> T {
        match &self.table {
            Some(t) => t[(cell.1 * self.per.0 + cell.0) as usize],
            None => (self.value)(cell),
        }
    }
}

/// `min + (max − min) × red`: a link to `constant` v over `[0, 1]` reads exactly v.
#[inline]
fn stretch(min: f32, max: f32, red: f32) -> f32 {
    min + (max - min) * red
}

#[derive(Default)]
pub struct Links<'a> {
    nodes: Vec<(&'a str, &'a Image)>,
}

impl<'a> Links<'a> {
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
        assert!(
            PerCell::for_pixels((4, 4), 64, read).table.is_some(),
            "16 × 4 ≤ 64"
        );
        assert!(
            PerCell::for_pixels((5, 5), 64, read).table.is_none(),
            "25 × 4 > 64"
        );
    }
}

#[cfg(test)]
#[path = "field_tests.rs"]
mod pixel_tests;
