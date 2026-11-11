//! The island check (#1892): whether a goal's piece of navmesh is cut off from
//! the flood's start, so the flood can drop it instead of running to its bound.
//!
//! Its answers depend on the navmesh alone, never on the query (no area mask, no
//! cost, and the start only decides which side of a closed piece it is on), so
//! the graph keeps them (`cache`) until the next bake (#2088): a
//! crate top or a big room is walked once per bake, not once per query.

use std::collections::VecDeque;

use super::moves::DIRS;
use crate::core::collections::{Map, Set};
use crate::navigation::{NavigationGraph, SpanRef};

/// The most walkable area, m², an island check visits before it calls a goal's
/// piece of navmesh "too big to be an island" and leaves it to the flood. A
/// 20 × 20 m roof fits; at the 0.25 m default cell that is 6400 spans, walked
/// once per region per bake (the answers are cached).
const ISLAND_AREA: f32 = 400.0;

/// The most spans the closed pieces may hold between bakes before the cache
/// forgets them and starts over: a bound on its memory, never on an answer.
const HELD_CAP: usize = 1 << 20;

/// It walks out from the goal over every edge that could lead **into** a span:
/// every span of a neighbouring column within a step (a two-way move, no
/// corner rule), every off-mesh link backwards and a two-way one forwards too,
/// no area mask. That is a superset of what can reach the goal, so a closed
/// piece without the start proves the goal unreachable.
#[derive(Default)]
pub(crate) struct Islands {
    /// The walk's cap, in spans: [`ISLAND_AREA`] over the cell's area.
    cap: usize,
    /// Off-mesh links as `(span, a span with a link into it)`, sorted; built
    /// the first time a check runs.
    links: Option<Vec<(u32, SpanRef, bool)>>,
    /// Closed pieces: every span that could walk into a goal, sorted, for each
    /// goal whose walk ended under the cap. A goal inside one is cut off from
    /// any start outside it.
    pub(super) pieces: Vec<Vec<u32>>,
    /// A piece holding each span, by span index (the first one found).
    pub(super) piece_of: Map<u32, u32>,
    held: usize,
    /// Spans proven **not** on an island, for every start: what walks into
    /// them is past the cap, or reaches such a span.
    pub(super) open: Set<u32>,
}

impl Islands {
    pub(super) fn new(nav: &NavigationGraph) -> Self {
        let cell = nav.grid_spacing.max(f32::EPSILON);
        Self {
            cap: ((ISLAND_AREA / (cell * cell)).ceil() as usize).max(1),
            ..Self::default()
        }
    }

    /// Whether `goal` lies on a piece of navmesh no larger than [`ISLAND_AREA`]
    /// that nothing on the start's side can walk into.
    pub(super) fn cut_off(&mut self, nav: &NavigationGraph, goal: SpanRef, start: SpanRef) -> bool {
        if self.open.contains(&goal.index) {
            return false;
        }
        if let Some(&p) = self.piece_of.get(&goal.index) {
            if self.pieces[p as usize].binary_search(&start.index).is_err() {
                return true;
            }
            // The start is in a piece holding the goal: the goal's own piece
            // may still be a smaller one without it, so walk it.
        }
        match self.walk(nav, goal) {
            Some(piece) => {
                let cut = piece.binary_search(&start.index).is_err();
                self.keep(piece);
                cut
            }
            None => false,
        }
    }

    /// Every span that could walk into `goal`, sorted, or `None` (caching the
    /// goal's region as open) when that is past the cap or reaches an open span.
    fn walk(&mut self, nav: &NavigationGraph, goal: SpanRef) -> Option<Vec<u32>> {
        let links = self.links.get_or_insert_with(|| link_ins(nav));
        // `tied` are the spans reached over two-way edges alone: the goal's
        // own region, all of it walkable to and from the goal. Only those may
        // be cached as open; a span past a one-way link may be a crate top
        // that drops into the goal's region but is an island itself. The tied
        // queue drains first, so a span is found tied whenever it can be.
        // It is breadth-first, so a capped walk keeps the disc round the goal
        // (about 11 m across at 0.25 m cells), not a depth-first snake: a later
        // goal near this one reaches the cached region in a few steps.
        let mut seen: Set<u32> = Set::default();
        let mut tied: Vec<u32> = vec![goal.index];
        seen.insert(goal.index);
        let mut tied_set: Set<u32> = Set::default();
        tied_set.insert(goal.index);
        let (mut tied_queue, mut loose_stack) = (VecDeque::from([goal]), Vec::new());
        while let Some((s, is_tied)) = tied_queue
            .pop_front()
            .map(|s| (s, true))
            .or_else(|| loose_stack.pop().map(|s| (s, false)))
        {
            if self.open.contains(&s.index) {
                self.open.extend(tied);
                return None;
            }
            let (two_way, one_way) = predecessors(nav, links, s);
            for (n, tie) in two_way
                .into_iter()
                .map(|n| (n, is_tied))
                .chain(one_way.into_iter().map(|n| (n, false)))
            {
                if tie && tied_set.insert(n.index) {
                    seen.insert(n.index);
                    tied.push(n.index);
                    tied_queue.push_back(n);
                } else if seen.insert(n.index) {
                    loose_stack.push(n);
                }
            }
            if seen.len() > self.cap {
                self.open.extend(tied);
                return None;
            }
        }
        let mut piece: Vec<u32> = seen.into_iter().collect();
        piece.sort_unstable();
        Some(piece)
    }

    /// Cache a closed piece, forgetting every piece first when the cache is full.
    fn keep(&mut self, piece: Vec<u32>) {
        if self.held + piece.len() > HELD_CAP {
            self.pieces.clear();
            self.piece_of.clear();
            self.held = 0;
        }
        let id = self.pieces.len() as u32;
        for &s in &piece {
            self.piece_of.entry(s).or_insert(id);
        }
        self.held += piece.len();
        self.pieces.push(piece);
    }
}

/// The spans with an edge into `s`: two-way ones (grid moves and two-way
/// links) and one-way ones (links that only lead into `s`).
fn predecessors(
    nav: &NavigationGraph,
    links: &[(u32, SpanRef, bool)],
    s: SpanRef,
) -> (Vec<SpanRef>, Vec<SpanRef>) {
    let a = nav.spans[s.index as usize];
    let first = links.partition_point(|l| l.0 < s.index);
    let (mut two_way, mut one_way) = (Vec::new(), Vec::new());
    for &(_, from, both) in links[first..].iter().take_while(|l| l.0 == s.index) {
        if both { &mut two_way } else { &mut one_way }.push(from);
    }
    for &(dx, dz, _) in &DIRS {
        let (gx, gz) = (s.gx + dx, s.gz + dz);
        for i in nav.span_range(gx, gz) {
            if nav.steps(a, nav.spans[i]) {
                two_way.push(SpanRef {
                    gx,
                    gz,
                    index: i as u32,
                });
            }
        }
    }
    (two_way, one_way)
}

/// Every off-mesh link as `(the span it leads into, the span it leaves,
/// two-way)`: a link into its `to` end always, into its `from` end too when it
/// is bidirectional. Sorted by the first.
fn link_ins(nav: &NavigationGraph) -> Vec<(u32, SpanRef, bool)> {
    let mut out = Vec::new();
    for link in nav.offmesh_links() {
        if let (Some(a), Some(b)) = (nav.link_span(link.from), nav.link_span(link.to)) {
            out.push((b.index, a, link.bidirectional));
            if link.bidirectional {
                out.push((a.index, b, true));
            }
        }
    }
    out.sort_unstable_by_key(|l| (l.0, l.1.index));
    out
}
