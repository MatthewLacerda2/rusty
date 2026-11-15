//! `scatter`'s stamps filed by where they reach (#2187): the tile split into a
//! square grid of buckets about half as wide as the widest stamp, each listing the
//! stamps that can cover a pixel in it, so a pixel visits those alone. A pixel
//! meets about `amount × (stamp width + bucket width)²` entries: half-width
//! buckets visit about half what stamp-wide ones do, for nine buckets a stamp.

/// The most buckets across: past it a bucket is narrower than a texel of the
/// largest bake's worth of tiny stamps would need, and only memory grows.
const MAX_GRID: usize = 256;

/// One stamp's copy in a bucket: the stamp's index among the drawn ones, and the
/// whole tiles its copy is shifted by (a stamp across the tile's edge is met from
/// the other side as the copy one tile over).
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Entry {
    pub stamp: u32,
    pub shift: [i8; 2],
}

/// The buckets, flattened: bucket `b`'s entries are `entries[starts[b]..starts[b + 1]]`,
/// in stamp order (so `over` lays them in index order with no sort).
pub(super) struct Buckets {
    g: usize,
    starts: Vec<u32>,
    entries: Vec<Entry>,
}

impl Buckets {
    /// File each stamp, given as its centre and its half-width (tiles, its square's
    /// reach either way on both axes), into every bucket its box overlaps.
    pub fn new(boxes: &[([f32; 2], f32)]) -> Self {
        let widest = boxes.iter().map(|b| b.1).fold(0.0, f32::max);
        let g = match widest > 0.0 {
            true => ((1.0 / widest) as usize).clamp(1, MAX_GRID),
            false => MAX_GRID,
        };
        let span = |c: f32, h: f32| {
            let lo = ((c - h) * g as f32).floor() as i64;
            lo..=((c + h) * g as f32).floor() as i64
        };
        let gi = g as i64;
        let each = |f: &mut dyn FnMut(usize, Entry)| {
            for (i, &(c, h)) in boxes.iter().enumerate() {
                for by in span(c[1], h) {
                    for bx in span(c[0], h) {
                        let bucket = (by.rem_euclid(gi) * gi + bx.rem_euclid(gi)) as usize;
                        let shift = [bx.div_euclid(gi) as i8, by.div_euclid(gi) as i8];
                        f(
                            bucket,
                            Entry {
                                stamp: i as u32,
                                shift,
                            },
                        );
                    }
                }
            }
        };
        let mut starts = vec![0u32; g * g + 1];
        each(&mut |b, _| starts[b + 1] += 1);
        for b in 0..g * g {
            starts[b + 1] += starts[b];
        }
        let mut next = starts.clone();
        let mut entries = vec![Entry::default(); starts[g * g] as usize];
        each(&mut |b, e| {
            entries[next[b] as usize] = e;
            next[b] += 1;
        });
        Self { g, starts, entries }
    }

    /// The stamps that can cover the point `(u, v)` of the tile (each in `[0, 1)`).
    #[inline]
    pub fn at(&self, u: f32, v: f32) -> &[Entry] {
        let g = self.g;
        let at = |c: f32| ((c * g as f32) as usize).min(g - 1);
        let b = at(v) * g + at(u);
        &self.entries[self.starts[b] as usize..self.starts[b + 1] as usize]
    }
}
