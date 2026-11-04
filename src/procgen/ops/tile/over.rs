//! `over`'s compositing (#1742): the stamps a pixel meets, laid one on another in
//! a fixed order, premultiplied.

use super::super::super::image_buf::Rgba;
use super::stamp::MAX_REACH;

/// The most stamps a pixel can meet: one per neighbour cell it looks at.
const SLOTS: usize = ((2 * MAX_REACH + 1) * (2 * MAX_REACH + 1)) as usize;

/// One stamp's sample at a pixel: its draw order, its colour (alpha unscaled) and
/// how much of what is under it it covers.
#[derive(Clone, Copy, Default)]
struct Hit {
    order: [i64; 4],
    colour: Rgba,
    coverage: f32,
}

/// The stamps one pixel meets, on the stack: nothing is allocated per pixel.
pub(super) struct Hits {
    hits: [Hit; SLOTS],
    len: usize,
}

impl Default for Hits {
    fn default() -> Self {
        Self {
            hits: [Hit::default(); SLOTS],
            len: 0,
        }
    }
}

impl Hits {
    /// Note a stamp's sample; `order` sorts it (lower is drawn first).
    pub fn push(&mut self, order: [i64; 4], colour: Rgba, coverage: f32) {
        self.hits[self.len] = Hit {
            order,
            colour,
            coverage,
        };
        self.len += 1;
    }

    /// Lay every noted stamp onto `out` in order: premultiplied over,
    /// `out = colour × alpha + out × (1 − coverage)`. The colour already carries the
    /// stamp's mask (it is the input times a value), so a lerp by coverage would
    /// darken a soft edge twice. Alpha keeps the larger, as `max` and `add` do.
    pub fn composite(&mut self, out: &mut Rgba) {
        let hits = &mut self.hits[..self.len];
        hits.sort_unstable_by_key(|h| h.order);
        for h in hits.iter() {
            let a = h.colour[3];
            for (o, c) in out[..3].iter_mut().zip(&h.colour[..3]) {
                *o = c * a + *o * (1.0 - h.coverage);
            }
            out[3] = out[3].max(a);
        }
    }
}

#[cfg(test)]
#[path = "over_tests.rs"]
mod tests;
