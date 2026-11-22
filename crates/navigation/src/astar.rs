//! Nodes are spans, not cells.

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
        // Ties break on the span index: byte-identical paths.
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

/// Detour's `maxNodes`; the longest complete Dust2 search took 45 074 expansions.
pub const MAX_SEARCH_NODES: u32 = 65_536;

struct Frontier<'p> {
    open_set: &'p mut BinaryHeap<NodeState>,
    pool: &'p mut NodePool,
}

/// Resumed only on the `NodePool` it began on, untouched by any other search meanwhile.
pub(crate) struct Search {
    open_set: BinaryHeap<NodeState>,
    closest: (f32, f32, SpanRef),
    goal: SpanRef,
    mask: u32,
    expanded: u32,
    cap: u32,
}

impl Search {
    pub(crate) fn remap(&mut self, remap: &dyn Fn(u32) -> Option<SpanRef>) -> bool {
        let node = |n: &NodeState| remap(n.node.index).map(|node| NodeState { node, ..*n });
        let moved: Option<Vec<_>> = self.open_set.iter().map(node).collect();
        let (Some(moved), Some(goal), Some(near)) =
            (moved, remap(self.goal.index), remap(self.closest.2.index))
        else {
            return false;
        };
        (self.open_set, self.goal, self.closest.2) = (moved.into(), goal, near);
        true
    }

    pub(crate) fn capped(&self) -> bool {
        self.expanded >= self.cap
    }
}

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
    pub fn get_next_path_step(&self, start: Vec3, target: Vec3) -> Vec3 {
        let path = self.calculate_path(start, target);
        path.corners.get(1).copied().unwrap_or(target)
    }

    /// A test hook: the span path other crates' navigation tests assert.
    pub fn path_between(&self, start: Vec3, target: Vec3) -> Option<Vec<SpanRef>> {
        let (path, complete) =
            self.find_path_masked(self.snap(start)?, self.snap(target)?, ALL_AREAS);
        complete.then_some(path)
    }

    /// Unreachable or capped: ends on the nearest reached span (Unity's `PathPartial`).
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

    /// `pool` must be begun for the graph's spans first, or the last search's slots read as
    /// reached.
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
            if current.g_score > frontier.pool.g(current.node.index) {
                continue;
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

    /// A diagonal pays its dearer cardinal's area cost, so no path shaves an area's corner;
    /// `flood::moves` mirrors this step.
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

fn relax(frontier: &mut Frontier, here: SpanRef, next: SpanRef, g: f32, goal: SpanRef) {
    if g < frontier.pool.g(next.index) {
        frontier.pool.set(next.index, g, Some(here));
        frontier.open_set.push(NodeState {
            node: next,
            g_score: g,
            f_score: g + heuristic(next, goal),
        });
    }
}

pub(super) fn heuristic(a: SpanRef, b: SpanRef) -> f32 {
    let dx = (a.gx - b.gx).abs() as f32;
    let dz = (a.gz - b.gz).abs() as f32;
    let (dmin, dmax) = (dx.min(dz), dx.max(dz));
    dmin * std::f32::consts::SQRT_2 + (dmax - dmin)
}

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
