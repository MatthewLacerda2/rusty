//! The walking areas under A\*'s eight grid steps, and the union-find both the
//! full build and the carry (`carry`) join them with.

use super::super::super::{NavigationGraph, SpanRef};

impl NavigationGraph {
    /// Each span's walking area under A\*'s steps (see the module doc), numbered in
    /// scan order, and how many there are.
    pub(super) fn step_areas(&self) -> (Vec<u32>, usize) {
        let mut roots = Roots::new(self.spans.len());
        let node = |s: SpanRef| s.index;
        for here in self.span_refs() {
            self.join_cardinals(here, &mut roots, node);
        }
        for here in self.span_refs() {
            self.join_diagonals(here, &mut roots, node);
        }
        let mut area = vec![u32::MAX; self.spans.len()];
        let mut count = 0;
        for i in 0..self.spans.len() as u32 {
            let r = roots.find(i) as usize;
            if area[r] == u32::MAX {
                (area[r], count) = (count as u32, count + 1);
            }
            area[i as usize] = area[r];
        }
        (area, count)
    }

    /// Join `here` with the spans its four cardinal steps reach, `node` naming each
    /// span's union-find node.
    pub(super) fn join_cardinals(
        &self,
        here: SpanRef,
        roots: &mut Roots,
        node: impl Fn(SpanRef) -> u32,
    ) {
        for (dx, dz) in CARDINAL {
            if let Some(next) = self.link_to(here, here.gx + dx, here.gz + dz) {
                roots.join(node(here), node(next));
            }
        }
    }

    /// Join `here` with the spans its diagonal steps reach. A diagonal is tried
    /// only into a cell holding a span of another set, so open floor, already
    /// joined by the cardinals, pays no more.
    pub(super) fn join_diagonals(
        &self,
        here: SpanRef,
        roots: &mut Roots,
        node: impl Fn(SpanRef) -> u32,
    ) {
        for (dx, dz) in DIAGONAL {
            let own = roots.find(node(here));
            let (gx, gz) = (here.gx + dx, here.gz + dz);
            let mut cell = self.span_range(gx, gz).map(|i| SpanRef {
                gx,
                gz,
                index: i as u32,
            });
            if cell.all(|s| roots.find(node(s)) == own) {
                continue;
            }
            if let Some(next) = self.neighbour(here, dx, dz) {
                roots.join(node(here), node(next));
            }
        }
    }
}

/// A union-find, each set rooted at its lowest node. A node holds its parent
/// plus one, 0 for a root, so a new one is a zeroed table: the carry (`carry`)
/// makes one as long as the navmesh and touches a few hundred of its nodes.
pub(super) struct Roots(Vec<u32>);

impl Roots {
    pub(super) fn new(nodes: usize) -> Self {
        Self(vec![0; nodes])
    }

    fn parent(&self, i: u32) -> u32 {
        self.0[i as usize].checked_sub(1).unwrap_or(i)
    }

    pub(super) fn find(&mut self, mut i: u32) -> u32 {
        loop {
            let p = self.parent(i);
            if p == i {
                return i;
            }
            let up = self.parent(p);
            self.0[i as usize] = up + 1;
            i = up;
        }
    }

    pub(super) fn join(&mut self, a: u32, b: u32) {
        let (a, b) = (self.find(a), self.find(b));
        if a != b {
            self.0[a.max(b) as usize] = a.min(b) + 1;
        }
    }
}

/// The grid steps A\* takes. A step can go one way only (`link_to` picks the
/// nearest floor, a diagonal needs its own two cardinals), so every span tries
/// all eight of its own.
const CARDINAL: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
const DIAGONAL: [(i32, i32); 4] = [(1, 1), (1, -1), (-1, 1), (-1, -1)];
