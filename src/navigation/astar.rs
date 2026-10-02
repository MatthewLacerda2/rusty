//! src/navigation/astar.rs — A\* over walkable spans (#454).
//!
//! Nodes are spans, not cells, so a path can climb a staircase onto the floor
//! directly above where it started. Edges are the 8-way moves `links.rs` allows;
//! the cost is horizontal travel plus the vertical climb, and the octile XZ
//! heuristic stays admissible because the climb only ever adds cost. Off-mesh
//! links (#462) are extra edges, costed never below the octile distance they span.
//!
//! Areas (#460): a step costs its distance times the mean cost of the two spans'
//! areas, a link its own cost (see `offmesh`), and a span or link whose area is not
//! in the query's mask is never entered (the start span is exempt, so an agent
//! standing in a forbidden area can still walk out). Every cost is at least 1, so
//! the heuristic stays admissible.

use super::{in_mask, NavSpan, NavigationGraph, SpanRef, ALL_AREAS};
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

/// The mutable A\* working set: dense per-span arrays (a span index is the node id),
/// cheaper than hashing on the long cross-level queries a bot issues.
struct Frontier {
    open_set: BinaryHeap<NodeState>,
    g_score: Vec<f32>,
    came_from: Vec<Option<SpanRef>>,
}

impl Frontier {
    fn new(spans: usize) -> Self {
        Self {
            open_set: BinaryHeap::new(),
            g_score: vec![f32::INFINITY; spans],
            came_from: vec![None; spans],
        }
    }

    fn g_of(&self, index: u32) -> f32 {
        self.g_score[index as usize]
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
    pub fn path_between(&self, start: Vec3, target: Vec3) -> Option<Vec<SpanRef>> {
        self.find_path(self.snap(start)?, self.snap(target)?)
    }

    /// A\* from span `start` to span `goal`, both ends included, or `None` when the
    /// goal is not reachable.
    pub fn find_path(&self, start: SpanRef, goal: SpanRef) -> Option<Vec<SpanRef>> {
        let (path, complete) = self.find_path_or_closest(start, goal);
        complete.then_some(path)
    }

    /// A\* from `start` toward `goal`. When the goal is unreachable, the path ends on
    /// the reached span nearest to it instead (Unity's `PathPartial`): least octile
    /// distance, then least cost, then lowest index, so the choice is deterministic.
    /// The flag says whether the path reaches `goal`.
    pub fn find_path_or_closest(&self, start: SpanRef, goal: SpanRef) -> (Vec<SpanRef>, bool) {
        self.find_path_masked(start, goal, ALL_AREAS)
    }

    /// [`Self::find_path_or_closest`] entering only the areas in `mask` (#460).
    pub fn find_path_masked(
        &self,
        start: SpanRef,
        goal: SpanRef,
        mask: u32,
    ) -> (Vec<SpanRef>, bool) {
        if start == goal {
            return (vec![start], true);
        }
        let mut frontier = Frontier::new(self.spans.len());
        frontier.g_score[start.index as usize] = 0.0;
        frontier.open_set.push(NodeState {
            node: start,
            g_score: 0.0,
            f_score: heuristic(start, goal),
        });
        let mut closest = (heuristic(start, goal), 0.0, start);

        while let Some(current) = frontier.open_set.pop() {
            if current.node == goal {
                return (reconstruct_path(&frontier.came_from, goal), true);
            }
            if current.g_score > frontier.g_of(current.node.index) {
                continue; // a stale heap entry
            }
            let h = heuristic(current.node, goal);
            let key = (h, current.g_score, current.node.index);
            if key < (closest.0, closest.1, closest.2.index) {
                closest = (h, current.g_score, current.node);
            }
            self.expand(current, goal, mask, &mut frontier);
        }
        (reconstruct_path(&frontier.came_from, closest.2), false)
    }

    /// Relax every span `current` can move to. The cardinal links are found once and
    /// reused as the corner-cutting check for the diagonals (`neighbour`'s rule).
    fn expand(&self, current: NodeState, goal: SpanRef, mask: u32, frontier: &mut Frontier) {
        let here = current.node;
        let NavSpan { y, area, .. } = self.spans[here.index as usize];
        let step = |dx: i32, dz: i32| {
            let to = self.link_to(here, here.gx + dx, here.gz + dz)?;
            in_mask(mask, self.spans[to.index as usize].area).then_some(to)
        };
        let (east, west, north, south) = (step(1, 0), step(-1, 0), step(0, 1), step(0, -1));
        for &(dx, dz, horiz) in &DIRS {
            let next = match (dx, dz) {
                (1, 0) => east,
                (-1, 0) => west,
                (0, 1) => north,
                (0, -1) => south,
                _ => {
                    let x_open = if dx > 0 { east } else { west }.is_some();
                    let z_open = if dz > 0 { north } else { south }.is_some();
                    if !(x_open && z_open) {
                        continue;
                    }
                    step(dx, dz)
                }
            };
            let Some(next) = next else {
                continue;
            };
            let to = self.spans[next.index as usize];
            let cost = 0.5 * (self.area_cost(area) + self.area_cost(to.area));
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
        frontier.came_from[next.index as usize] = Some(here);
        frontier.g_score[next.index as usize] = g;
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
fn reconstruct_path(came_from: &[Option<SpanRef>], goal: SpanRef) -> Vec<SpanRef> {
    let mut path = vec![goal];
    let mut curr = goal;
    while let Some(prev) = came_from[curr.index as usize] {
        path.push(prev);
        curr = prev;
    }
    path.reverse();
    path
}
