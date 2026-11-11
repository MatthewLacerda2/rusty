//! src/navigation/astar.rs — A\* over walkable spans (#454).
//!
//! Nodes are spans, not cells, so a path can climb a staircase onto the floor
//! directly above where it started. Edges are the 8-way moves `links.rs` allows;
//! the cost is horizontal travel plus the vertical climb, and the octile XZ
//! heuristic stays admissible because the climb only ever adds cost. Off-mesh
//! links (#462) are extra edges, costed never below the octile distance they span.
//!
//! Areas (#460): a step costs its distance times the mean cost of the two spans'
//! areas (a diagonal at least the dearer cardinal it cuts past), a link its own cost (see `offmesh`), and a span or link whose area is not
//! in the query's mask is never entered (the start span is exempt, so an agent
//! standing in a forbidden area can still walk out). Every cost is at least 1, so
//! the heuristic stays admissible.
//!
//! A search is resumable (#2087): [`NavigationGraph::begin_search`] opens it and
//! [`NavigationGraph::step_search`] spends a budget of node expansions on it, so the
//! agents' path queue (`request`) can spread one search over several ticks. Every
//! search, sliced or not, gives up after [`MAX_SEARCH_NODES`] expansions (far fewer
//! toward a goal on an island it provably cannot reach, `request::islands`) with the
//! partial path to the nearest span it reached (Detour's `maxNodes`), so an
//! unreachable goal costs a bounded amount instead of flooding the whole navmesh.

use super::pool::NodePool;
use super::{in_mask, NavSpan, NavigationGraph, SpanRef, ALL_AREAS, MIN_AREA_COST};
use glam::Vec3;
use std::collections::BinaryHeap;

#[derive(Copy, Clone, PartialEq)]
struct NodeState {
    node: SpanRef,
    g_score: f32,
    f_score: f32,
}

impl Eq for NodeState {}

impl Ord for NodeState {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Min-heap on f_score (BinaryHeap is a max-heap, so reverse). Ties break on
        // the span index, a fixed total order, so the popped sequence — and the path —
        // is byte-identical across runs.
        other
            .f_score
            .partial_cmp(&self.f_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(other.node.index.cmp(&self.node.index))
    }
}

impl PartialOrd for NodeState {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Expansions after which a search stops and returns the partial path to the span
/// nearest its goal (Detour's `maxNodes`, #2087). The longest complete search the
/// Dust2 arena's 32 bots ran in a minute expanded 45 074 of its 181 626 spans; the
/// cap leaves headroom over that and bounds what any one search costs.
pub const MAX_SEARCH_NODES: u32 = 65_536;

/// The mutable A\* working set: the open heap, and the [`NodePool`] for each span's
/// best cost and parent (a span index is the node id).
struct Frontier<'p> {
    open_set: &'p mut BinaryHeap<NodeState>,
    pool: &'p mut NodePool,
}

impl Frontier<'_> {
    fn g_of(&self, index: u32) -> f32 {
        self.pool.g(index)
    }
}

/// One A\* search in progress (#2087): its heap, the span nearest the goal so far,
/// and how many expansions it spent. Its costs and parents live in the [`NodePool`]
/// it was begun on, which must be handed back, untouched by any other search, to
/// every [`NavigationGraph::step_search`].
pub(crate) struct Search {
    open_set: BinaryHeap<NodeState>,
    /// (octile distance to the goal, cost, span) of the nearest span expanded.
    closest: (f32, f32, SpanRef),
    goal: SpanRef,
    mask: u32,
    expanded: u32,
    /// The expansions it may spend (`NavigationGraph::search_cap`).
    cap: u32,
}

#[cfg(test)]
impl Search {
    /// The expansions this search has spent.
    pub(crate) fn expanded(&self) -> u32 {
        self.expanded
    }
}

/// The 8-way moves: (dx, dz, horizontal cost in cells).
const DIRS: [(i32, i32, f32); 8] = [
    (1, 0, 1.0),
    (-1, 0, 1.0),
    (0, 1, 1.0),
    (0, -1, 1.0),
    (1, 1, std::f32::consts::SQRT_2),
    (1, -1, std::f32::consts::SQRT_2),
    (-1, 1, std::f32::consts::SQRT_2),
    (-1, -1, std::f32::consts::SQRT_2),
];

impl NavigationGraph {
    /// The next corner of the smoothed path from `start` to `target`
    /// ([`Self::calculate_path`]), carrying the floor height so the caller follows
    /// stairs and ramps in `y`. With no path it returns `target`, the historical beeline.
    pub fn get_next_path_step(&self, start: Vec3, target: Vec3) -> Vec3 {
        let path = self.calculate_path(start, target);
        path.corners.get(1).copied().unwrap_or(target)
    }

    /// The span path from the span `start` snaps to to the one `target` snaps to.
    /// `None` when either end is off the navmesh or the target is not reachable.
    /// Test seam for `tests/navigation_layered/building.rs`.
    pub fn path_between(&self, start: Vec3, target: Vec3) -> Option<Vec<SpanRef>> {
        let (path, complete) =
            self.find_path_masked(self.snap(start)?, self.snap(target)?, ALL_AREAS);
        complete.then_some(path)
    }

    /// A\* from span `start` toward span `goal`, both ends included, entering only
    /// the areas in `mask` (#460). When the goal is unreachable, or the search hits
    /// its cap ([`MAX_SEARCH_NODES`], less toward an island), the path ends on the
    /// reached span nearest to it instead (Unity's `PathPartial`): least octile
    /// distance, then least cost, then lowest index, so the choice is deterministic. The flag says whether the path reaches
    /// `goal`.
    pub fn find_path_masked(
        &self,
        start: SpanRef,
        goal: SpanRef,
        mask: u32,
    ) -> (Vec<SpanRef>, bool) {
        self.search.with(self.spans.len(), |pool| {
            let mut search = self.begin_search(start, goal, mask, pool);
            let mut unbounded = u32::MAX;
            let found = self.step_search(&mut search, pool, &mut unbounded);
            found.expect("an unbounded budget runs the search to its end")
        })
    }

    /// Open a search from `start` to `goal` on `pool`, which must be freshly begun
    /// for this graph's spans (`NodePool::begin`).
    pub(crate) fn begin_search(
        &self,
        start: SpanRef,
        goal: SpanRef,
        mask: u32,
        pool: &mut NodePool,
    ) -> Search {
        pool.set(start.index, 0.0, None);
        let h = heuristic(start, goal);
        let mut open_set = BinaryHeap::new();
        open_set.push(NodeState {
            node: start,
            g_score: 0.0,
            f_score: h,
        });
        Search {
            open_set,
            closest: (h, 0.0, start),
            goal,
            mask,
            expanded: 0,
            cap: self.search_cap(start, goal),
        }
    }

    /// Run `search` for at most `budget` expansions, taking what it spends off
    /// `budget`. `Some` with the span path and whether it reaches the goal once the
    /// search ends; `None` when the budget ran out first, to be resumed later on the
    /// same `pool`. Resuming pops the same heap in the same order, so a sliced search
    /// finds exactly the path an unsliced one does.
    pub(crate) fn step_search(
        &self,
        search: &mut Search,
        pool: &mut NodePool,
        budget: &mut u32,
    ) -> Option<(Vec<SpanRef>, bool)> {
        let (goal, mask) = (search.goal, search.mask);
        let mut frontier = Frontier {
            open_set: &mut search.open_set,
            pool,
        };
        while search.expanded < search.cap {
            if *budget == 0 {
                return None;
            }
            let Some(current) = frontier.open_set.pop() else {
                break;
            };
            if current.node == goal {
                return Some((reconstruct_path(frontier.pool, goal), true));
            }
            if current.g_score > frontier.g_of(current.node.index) {
                continue; // a stale heap entry
            }
            let h = heuristic(current.node, goal);
            let c = search.closest;
            if (h, current.g_score, current.node.index) < (c.0, c.1, c.2.index) {
                search.closest = (h, current.g_score, current.node);
            }
            self.expand(current, goal, mask, &mut frontier);
            search.expanded += 1;
            *budget -= 1;
        }
        Some((reconstruct_path(frontier.pool, search.closest.2), false))
    }

    /// Relax every span `current` can move to. The cardinal links are found once and
    /// reused as the corner-cutting check for the diagonals (`neighbour`'s rule). A
    /// diagonal cuts past both cardinal cells, so it pays at least the dearer of their
    /// area costs: a path never shaves the corner of an area it routes around.
    fn expand(&self, current: NodeState, goal: SpanRef, mask: u32, frontier: &mut Frontier) {
        let here = current.node;
        let NavSpan { y, area, .. } = self.spans[here.index as usize];
        let step = |dx: i32, dz: i32| {
            let to = self.link_to(here, here.gx + dx, here.gz + dz)?;
            in_mask(mask, self.spans[to.index as usize].area).then_some(to)
        };
        let (east, west, north, south) = (step(1, 0), step(-1, 0), step(0, 1), step(0, -1));
        let cost_of = |s: SpanRef| self.area_cost(self.spans[s.index as usize].area);
        for &(dx, dz, horiz) in &DIRS {
            let (next, corner) = match (dx, dz) {
                (1, 0) => (east, MIN_AREA_COST),
                (-1, 0) => (west, MIN_AREA_COST),
                (0, 1) => (north, MIN_AREA_COST),
                (0, -1) => (south, MIN_AREA_COST),
                _ => {
                    let x = if dx > 0 { east } else { west };
                    let z = if dz > 0 { north } else { south };
                    let (Some(x), Some(z)) = (x, z) else {
                        continue;
                    };
                    (step(dx, dz), cost_of(x).max(cost_of(z)))
                }
            };
            let Some(next) = next else {
                continue;
            };
            let to = self.spans[next.index as usize];
            let cost = (0.5 * (self.area_cost(area) + cost_of(next))).max(corner);
            let g = current.g_score + (horiz + (to.y - y).abs()) * cost;
            relax(frontier, here, next, g, goal);
        }
        for link in self.link_moves(here) {
            let lands = self.spans[link.to.index as usize].area;
            if in_mask(mask, link.area) && in_mask(mask, lands) {
                relax(frontier, here, link.to, current.g_score + link.cost, goal);
            }
        }
    }
}

/// Record `next` as reached from `here` at cost `g` when that beats its best so far.
fn relax(frontier: &mut Frontier, here: SpanRef, next: SpanRef, g: f32, goal: SpanRef) {
    if g < frontier.g_of(next.index) {
        frontier.pool.set(next.index, g, Some(here));
        frontier.open_set.push(NodeState {
            node: next,
            g_score: g,
            f_score: g + heuristic(next, goal),
        });
    }
}

/// Octile distance in cells: an admissible lower bound for 8-way moves.
pub(super) fn heuristic(a: SpanRef, b: SpanRef) -> f32 {
    let dx = (a.gx - b.gx).abs() as f32;
    let dz = (a.gz - b.gz).abs() as f32;
    let (dmin, dmax) = (dx.min(dz), dx.max(dz));
    dmin * std::f32::consts::SQRT_2 + (dmax - dmin)
}

/// Walk the `came_from` chain back from `goal`, returning the path start→goal.
fn reconstruct_path(pool: &NodePool, goal: SpanRef) -> Vec<SpanRef> {
    let mut path = vec![goal];
    let mut curr = goal;
    while let Some(prev) = pool.came_from(curr.index) {
        path.push(prev);
        curr = prev;
    }
    path.reverse();
    path
}
