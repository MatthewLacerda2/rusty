use super::super::super::image_buf::Rgba;
use super::stamp::MAX_REACH;

const SLOTS: usize = ((2 * MAX_REACH + 1) * (2 * MAX_REACH + 1)) as usize;

#[derive(Clone, Copy, Default)]
struct Hit {
    order: [i64; 4],
    colour: Rgba,
    coverage: f32,
}

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
