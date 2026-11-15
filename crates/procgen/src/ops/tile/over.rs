//! `over`'s compositing (#1742): the stamps a pixel meets, laid one on another in
//! a fixed order, premultiplied.

use super::super::super::image_buf::Rgba;
use super::stamp::MAX_REACH;

/// The stamps a pixel keeps on the stack: one per neighbour cell it looks at in a
/// square grid. A turned stamp in a flat cell reaches more rows, and a pixel
/// meeting more than this spills the rest to the heap.
const SLOTS: usize = ((2 * MAX_REACH + 1) * (2 * MAX_REACH + 1)) as usize;

/// One stamp's sample at a pixel: its draw order, its colour (alpha unscaled) and
/// how much of what is under it it covers.
#[derive(Clone, Copy, Default)]
struct Hit {
    order: [i64; 4],
    colour: Rgba,
    coverage: f32,
}

/// The stamps one pixel meets, on the stack: nothing is allocated per pixel
/// unless it meets more than [`SLOTS`] of them.
pub(super) struct Hits {
    hits: [Hit; SLOTS],
    len: usize,
    spill: Vec<Hit>,
}

impl Default for Hits {
    fn default() -> Self {
        Self {
            hits: [Hit::default(); SLOTS],
            len: 0,
            spill: Vec::new(),
        }
    }
}

impl Hits {
    /// Note a stamp's sample; `order` sorts it (lower is drawn first).
    pub fn push(&mut self, order: [i64; 4], colour: Rgba, coverage: f32) {
        let hit = Hit {
            order,
            colour,
            coverage,
        };
        match self.len < SLOTS {
            true => {
                self.hits[self.len] = hit;
                self.len += 1;
            }
            false => self.spill.push(hit),
        }
    }

    /// Lay every noted stamp onto `out` in order: premultiplied over,
    /// `out = colour × alpha + out × (1 − coverage)`. The colour already carries the
    /// stamp's mask (it is the input times a value), so a lerp by coverage would
    /// darken a soft edge twice. Alpha keeps the larger, as `max` and `add` do.
    pub fn composite(&mut self, out: &mut Rgba) {
        let hits = match self.spill.is_empty() {
            true => &mut self.hits[..self.len],
            false => {
                self.spill.extend_from_slice(&self.hits[..self.len]);
                &mut self.spill[..]
            }
        };
        hits.sort_unstable_by_key(|h| h.order);
        for h in hits.iter() {
            over(out, h.colour, h.coverage);
        }
    }
}

/// Lay one stamp's `colour` (covering `coverage` of what is under it) onto `out`,
/// premultiplied: what [`Hits::composite`] does per stamp, and `scatter` does in its
/// stamps' index order.
#[inline]
pub(in crate::ops) fn over(out: &mut Rgba, colour: Rgba, coverage: f32) {
    let a = colour[3];
    for (o, c) in out[..3].iter_mut().zip(&colour[..3]) {
        *o = c * a + *o * (1.0 - coverage);
    }
    out[3] = out[3].max(a);
}

#[cfg(test)]
#[path = "over_tests.rs"]
mod tests;
