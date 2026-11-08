//! The island check (#1892): whether a goal's piece of navmesh is cut off from
//! the flood's start, so the flood can drop it instead of running to its bound.

use std::collections::VecDeque;

use super::DIRS;
use crate::core::collections::Set;
use crate::navigation::{NavigationGraph, SpanRef};

/// The most walkable area, m², an island check visits before it calls a goal's
/// piece of navmesh "too big to be an island" and leaves it to the flood. A
/// 20 × 20 m roof fits; at the 0.25 m default cell that is 6400 spans, walked
/// once per region per query (both answers are cached).
const ISLAND_AREA: f32 = 400.0;

/// It walks out from the goal over every edge that could lead **into** a span:
/// every span of a neighbouring column within a step (a two-way move, no
/// corner rule), every off-mesh link backwards and a two-way one forwards too,
/// no area mask. That is a superset of what can reach the goal, so a closed
/// piece without the start proves the goal unreachable.
pub(super) struct Islands<'a> {
    nav: &'a NavigationGraph,
    /// The walk's cap, in spans: [`ISLAND_AREA`] over the cell's area.
    cap: usize,
    /// Off-mesh links as `(span, a span with a link into it)`, sorted; built
    /// the first time a check runs.
    links: Option<Vec<(u32, SpanRef, bool)>>,
    /// Spans proven cut off: everything that reaches them is in a cut-off piece.
    pub(super) cut: Set<u32>,
    /// Spans proven **not** cut off (what reaches them holds the start, or is
    /// past the cap): a goal that is one, or reaches one, is answered at once.
    pub(super) open: Set<u32>,
}

impl<'a> Islands<'a> {
    pub(super) fn new(nav: &'a NavigationGraph) -> Self {
        let cell = nav.grid_spacing.max(f32::EPSILON);
        Self {
            nav,
            cap: ((ISLAND_AREA / (cell * cell)).ceil() as usize).max(1),
            links: None,
            cut: Set::default(),
            open: Set::default(),
        }
    }

    /// Whether `goal` lies on a piece of navmesh no larger than [`ISLAND_AREA`]
    /// that nothing on the start's side can walk into.
    pub(super) fn cut_off(&mut self, goal: SpanRef, start: SpanRef) -> bool {
        if self.cut.contains(&goal.index) {
            return true;
        }
        let nav = self.nav;
        let links = self.links.get_or_insert_with(|| link_ins(nav));
        // `tied` are the spans reached over two-way edges alone: the goal's
        // own region, all of it walkable to and from the goal. Only those may
        // be cached as "not cut off"; a span past a one-way link may be a crate
        // top that drops into the goal's region but is an island itself. The
        // tied queue drains first, so a span is found tied whenever it can be.
        // It is breadth-first, so a capped walk keeps the disc round the goal
        // (about 11 m across at 0.25 m cells), not a depth-first snake: a later
        // goal near this one reaches the cached region in a few steps.
        let mut seen: Set<u32> = Set::default();
        let mut tied: Set<u32> = Set::default();
        seen.insert(goal.index);
        tied.insert(goal.index);
        let (mut tied_queue, mut loose_stack) = (VecDeque::from([goal]), Vec::new());
        let mut not_cut = false;
        while let Some((s, is_tied)) = tied_queue
            .pop_front()
            .map(|s| (s, true))
            .or_else(|| loose_stack.pop().map(|s| (s, false)))
        {
            if s.index == start.index || self.open.contains(&s.index) {
                not_cut = true;
                break;
            }
            let (two_way, one_way) = predecessors(nav, links, s);
            for (n, tie) in two_way
                .into_iter()
                .map(|n| (n, is_tied))
                .chain(one_way.into_iter().map(|n| (n, false)))
            {
                if tie && tied.insert(n.index) {
                    seen.insert(n.index);
                    tied_queue.push_back(n);
                } else if seen.insert(n.index) {
                    loose_stack.push(n);
                }
            }
            if seen.len() > self.cap {
                not_cut = true;
                break;
            }
        }
        if not_cut {
            self.open.extend(tied);
        } else {
            self.cut.extend(seen);
        }
        !not_cut
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
        let run = nav.run_between(s, gx, gz);
        for i in nav.span_range(gx, gz) {
            if nav.steps(a, nav.spans[i], run) {
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
