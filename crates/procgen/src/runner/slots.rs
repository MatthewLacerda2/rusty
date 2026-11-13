//! crates/procgen/src/runner/slots.rs — the images a run holds between nodes, and what
//! they cost (#2002's review): how many are alive at once and how many bytes, at
//! the two moments a node's run peaks (while its op runs, and while its output
//! packs to one channel).

use std::borrow::Cow;

use super::held::Held;
use crate::image_buf::Image;

/// Bytes of one RGBA f32 image.
pub(super) fn image_bytes(img: &Image) -> usize {
    std::mem::size_of_val(img.pixels())
}

/// Each node's held image, with the run's peaks.
pub(super) struct Slots {
    slots: Vec<Option<Held>>,
    live: usize,
    held_bytes: usize,
    pub peak_live: usize,
    pub peak_bytes: usize,
}

impl Slots {
    pub fn new(nodes: usize) -> Self {
        Self {
            slots: (0..nodes).map(|_| None).collect(),
            live: 0,
            held_bytes: 0,
            peak_live: 0,
            peak_bytes: 0,
        }
    }

    pub fn get(&self, i: usize) -> Option<&Held> {
        self.slots[i].as_ref()
    }

    pub fn put(&mut self, i: usize, held: Held) {
        self.live += 1;
        self.held_bytes += held.bytes();
        self.slots[i] = Some(held);
    }

    pub fn take(&mut self, i: usize) -> Option<Held> {
        let held = self.slots[i].take()?;
        self.live -= 1;
        self.held_bytes -= held.bytes();
        Some(held)
    }

    /// A node's op is running: `images` more are alive beside the held ones (its
    /// rebuilt or taken inputs and its output), `bytes` more bytes.
    pub fn note(&mut self, images: usize, bytes: usize) {
        self.peak_live = self.peak_live.max(self.live + images);
        self.peak_bytes = self.peak_bytes.max(self.held_bytes + bytes);
    }

    /// The images node `i` reads at `ids` (inputs then links, repeats included),
    /// as views of each distinct one once, and for each entry of `ids` the index of
    /// its view. An input this node is the last reader of (`uses`), and no target,
    /// is taken out of its slot instead of copied beside it.
    pub fn gather(&mut self, ids: &[usize], uses: &[usize], targets: &[usize]) -> Gathered<'_> {
        let mut distinct: Vec<usize> = Vec::new();
        for &j in ids {
            if !distinct.contains(&j) {
                distinct.push(j);
            }
        }
        let taken: Vec<Option<Image>> = distinct
            .iter()
            .map(|&j| {
                let reads = ids.iter().filter(|&&k| k == j).count();
                let last = uses[j] == reads && !targets.contains(&j);
                last.then(|| self.take(j).map(Held::into_image)).flatten()
            })
            .collect();
        let views = distinct
            .iter()
            .zip(taken)
            .map(|(&j, own)| match own {
                Some(img) => Cow::Owned(img),
                None => self.get(j).expect("plan orders inputs first").view(),
            })
            .collect();
        let index = ids
            .iter()
            .map(|j| distinct.iter().position(|d| d == j).unwrap_or(0))
            .collect();
        Gathered { views, index }
    }

    pub fn into_inner(self) -> Vec<Option<Held>> {
        self.slots
    }
}

/// A node's inputs as [`Slots::gather`] hands them out.
pub(super) struct Gathered<'a> {
    views: Vec<Cow<'a, Image>>,
    index: Vec<usize>,
}

impl Gathered<'_> {
    /// One image per entry of the gathered ids.
    pub fn edges(&self) -> Vec<&Image> {
        self.index.iter().map(|&k| self.views[k].as_ref()).collect()
    }

    /// The images and bytes the views own (rebuilt or taken, not borrowed).
    pub fn owned(&self) -> (usize, usize) {
        let owned = self.views.iter().filter_map(|v| match v {
            Cow::Owned(img) => Some(image_bytes(img)),
            Cow::Borrowed(_) => None,
        });
        owned.fold((0, 0), |(n, b), bytes| (n + 1, b + bytes))
    }
}
