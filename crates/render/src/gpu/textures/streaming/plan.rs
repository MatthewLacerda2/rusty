//! Each streamed texture's levels to keep.

use std::collections::BinaryHeap;

/// Levels kept above the screen size's need: a tiled surface reads finer than its bounds.
const MARGIN: f32 = 1.0;

pub(crate) struct Candidate {
    /// In texels.
    pub side: u32,
    pub dropped: u32,
    pub most: u32,
    /// Its largest use this frame, in pixels; `None` unseen.
    pub pixels: Option<f32>,
    pub levels: Vec<u64>,
}

impl Candidate {
    pub(crate) fn bytes(&self, dropped: u32) -> u64 {
        self.levels[dropped as usize..].iter().sum()
    }

    /// Down only when two levels over its need.
    fn wanted(&self) -> u32 {
        let Some(pixels) = self.pixels.filter(|p| !p.is_nan()) else {
            return self.dropped;
        };
        let ratio = self.side as f32 / pixels.max(1.0);
        let need = (ratio.log2().floor() - MARGIN).clamp(0.0, self.most as f32) as u32;
        match need > self.dropped && need < self.dropped + 2 {
            true => self.dropped,
            false => need,
        }
    }

    fn excess(&self, dropped: u32) -> f32 {
        match self.pixels {
            None => f32::INFINITY,
            Some(px) => (self.side >> dropped) as f32 / px.max(1.0),
        }
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct Plan {
    pub targets: Vec<u32>,
    pub over: bool,
}

/// In bytes; `others` are the textures that do not stream.
pub(crate) fn plan(candidates: &[Candidate], others: u64, budget: Option<u64>) -> Plan {
    let mut targets: Vec<u32> = candidates.iter().map(Candidate::wanted).collect();
    let Some(budget) = budget else {
        return Plan {
            targets,
            over: false,
        };
    };
    let mut total = others
        + (candidates.iter().zip(&targets))
            .map(|(c, &t)| c.bytes(t))
            .sum::<u64>();
    let key = |at: usize, t: u32| (Excess(candidates[at].excess(t)), std::cmp::Reverse(at));
    let mut heap: BinaryHeap<_> = (0..candidates.len())
        .filter(|&at| targets[at] < candidates[at].most)
        .map(|at| key(at, targets[at]))
        .collect();
    while total > budget {
        let Some((_, std::cmp::Reverse(at))) = heap.pop() else {
            break;
        };
        total -= candidates[at].levels[targets[at] as usize];
        targets[at] += 1;
        if targets[at] < candidates[at].most {
            heap.push(key(at, targets[at]));
        }
    }
    Plan {
        targets,
        over: total > budget,
    }
}

#[derive(Clone, Copy)]
struct Excess(f32);

impl PartialEq for Excess {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}

impl Eq for Excess {}

impl PartialOrd for Excess {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Excess {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}
