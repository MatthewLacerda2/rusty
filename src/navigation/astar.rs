//! src/navigation/astar.rs — A\* over walkable spans (#454).
//!
//! Nodes are spans, not cells, so a path can climb a staircase onto the floor
//! directly above where it started. Edges are the 8-way moves `links.rs` allows;
//! the cost is horizontal travel plus the vertical climb, and the octile XZ
//! heuristic stays admissible because the climb only ever adds cost.

use super::{NavigationGraph, SpanRef};
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
    /// The next point along the shortest path from `start` to `target`, carrying the
    /// span's floor height so the caller follows stairs and ramps in `y`. Both ends
    /// snap to spans (`snap.rs`); with no path (or nothing to snap to) it returns
    /// `target`, the historical beeline.
    pub fn get_next_path_step(&self, start: Vec3, target: Vec3) -> Vec3 {
        match self.path_between(start, target) {
            Some(path) if path.len() > 1 => self.span_world(path[1]),
            _ => target,
        }
    }

    /// The span path from the span `start` snaps to to the one `target` snaps to.
    pub fn path_between(&self, start: Vec3, target: Vec3) -> Option<Vec<SpanRef>> {
        self.find_path(self.snap(start)?, self.snap(target)?)
    }

    /// A\* from span `start` to span `goal`, both ends included, or `None` when the
    /// goal is not reachable.
    pub fn find_path(&self, start: SpanRef, goal: SpanRef) -> Option<Vec<SpanRef>> {
        if start == goal {
            return Some(vec![start]);
        }
        let mut frontier = Frontier::new(self.spans.len());
        frontier.g_score[start.index as usize] = 0.0;
        frontier.open_set.push(NodeState {
            node: start,
            g_score: 0.0,
            f_score: heuristic(start, goal),
        });

        while let Some(current) = frontier.open_set.pop() {
            if current.node == goal {
                return Some(reconstruct_path(&frontier.came_from, goal));
            }
            if current.g_score > frontier.g_of(current.node.index) {
                continue; // a stale heap entry
            }
            self.expand(current, goal, &mut frontier);
        }
        None
    }

    /// Relax every span `current` can move to. The cardinal links are found once and
    /// reused as the corner-cutting check for the diagonals (`neighbour`'s rule).
    fn expand(&self, current: NodeState, goal: SpanRef, frontier: &mut Frontier) {
        let here = current.node;
        let y = self.spans[here.index as usize].y;
        let card = |dx: i32, dz: i32| self.link_to(here, here.gx + dx, here.gz + dz);
        let (east, west, north, south) = (card(1, 0), card(-1, 0), card(0, 1), card(0, -1));
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
                    self.link_to(here, here.gx + dx, here.gz + dz)
                }
            };
            let Some(next) = next else {
                continue;
            };
            let dh = (self.spans[next.index as usize].y - y).abs();
            let tentative_g = current.g_score + horiz + dh;
            if tentative_g < frontier.g_of(next.index) {
                frontier.came_from[next.index as usize] = Some(here);
                frontier.g_score[next.index as usize] = tentative_g;
                frontier.open_set.push(NodeState {
                    node: next,
                    g_score: tentative_g,
                    f_score: tentative_g + heuristic(next, goal),
                });
            }
        }
    }
}

/// Octile distance in cells: an admissible lower bound for 8-way moves.
fn heuristic(a: SpanRef, b: SpanRef) -> f32 {
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
