//! A stamp lands in at most 3 × 3 buckets of its level; a pixel meets about four times the stamps
//! of its own size that could cover it, whatever the others' sizes.

/// The finest grid is 256 buckets across; past it only memory grows.
const LEVELS: usize = 9;

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Entry {
    pub stamp: u32,
    pub shift: [i8; 2],
}

#[derive(Default)]
struct Level {
    g: usize,
    starts: Vec<u32>,
    entries: Vec<Entry>,
}

impl Level {
    fn new(g: usize, boxes: &[(u32, [f32; 2], f32)]) -> Self {
        let gi = g as i64;
        let span = |c: f32, h: f32| {
            let lo = ((c - h) * g as f32).floor() as i64;
            lo..=((c + h) * g as f32).floor() as i64
        };
        let each = |f: &mut dyn FnMut(usize, Entry)| {
            for &(stamp, c, h) in boxes {
                for by in span(c[1], h) {
                    for bx in span(c[0], h) {
                        let bucket = (by.rem_euclid(gi) * gi + bx.rem_euclid(gi)) as usize;
                        let shift = [bx.div_euclid(gi) as i8, by.div_euclid(gi) as i8];
                        f(bucket, Entry { stamp, shift });
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

    #[inline]
    fn at(&self, u: f32, v: f32) -> &[Entry] {
        let g = self.g;
        if g == 0 {
            return &[];
        }
        let at = |c: f32| ((c * g as f32) as usize).min(g - 1);
        let b = at(v) * g + at(u);
        &self.entries[self.starts[b] as usize..self.starts[b + 1] as usize]
    }
}

fn level(h: f32) -> usize {
    let mut k = 0;
    while k + 1 < LEVELS && h * (1u32 << (k + 1)) as f32 <= 1.0 {
        k += 1;
    }
    k
}

pub(super) struct Buckets {
    levels: [Level; LEVELS],
}

impl Buckets {
    /// Each box is a stamp's centre and half-width (tiles, its square's
    /// reach either way on both axes). The half-width is padded by a hair, so a
    /// texel the stamp covers by a rounding of its own math is never in a bucket
    /// the stamp missed.
    pub fn new(boxes: &[([f32; 2], f32)]) -> Self {
        let mut by_level: [Vec<(u32, [f32; 2], f32)>; LEVELS] = Default::default();
        for (i, &(c, h)) in boxes.iter().enumerate() {
            let h = h * (1.0 + 1e-4) + 1e-6;
            by_level[level(h)].push((i as u32, c, h));
        }
        let mut levels: [Level; LEVELS] = Default::default();
        for (k, boxes) in by_level.iter().enumerate() {
            if !boxes.is_empty() {
                levels[k] = Level::new(1 << k, boxes);
            }
        }
        Self { levels }
    }

    /// In stamp order: `scatter` lays the copies `over` in that order.
    #[inline]
    pub fn each(&self, u: f32, v: f32, mut f: impl FnMut(Entry)) {
        let mut lists: [&[Entry]; LEVELS] = [&[]; LEVELS];
        let mut n = 0;
        for level in &self.levels {
            let list = level.at(u, v);
            if !list.is_empty() {
                lists[n] = list;
                n += 1;
            }
        }
        if n == 1 {
            return lists[0].iter().for_each(|&e| f(e));
        }
        let lists = &mut lists[..n];
        while let Some(list) = lists
            .iter_mut()
            .filter(|l| !l.is_empty())
            .min_by_key(|l| l[0].stamp)
        {
            f(list[0]);
            *list = &list[1..];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stamp_files_in_the_finest_level_at_least_as_wide() {
        assert_eq!(level(1.2), 0);
        assert_eq!(level(0.5), 1);
        assert_eq!(level(0.3), 1);
        assert_eq!(level(0.25), 2);
        assert_eq!(level(0.0), LEVELS - 1);
    }

    #[test]
    fn a_pixel_meets_the_stamps_of_every_level_in_stamp_order() {
        let boxes = [
            ([0.5, 0.5], 0.6),
            ([0.511, 0.509], 0.001),
            ([0.5, 0.52], 0.1),
            ([0.9, 0.9], 0.001),
            ([0.51, 0.51], 0.002),
        ];
        let b = Buckets::new(&boxes);
        let mut met = Vec::new();
        b.each(0.51, 0.51, |e| met.push(e.stamp));
        met.dedup();
        assert_eq!(met, [0, 1, 2, 4]);
    }
}
