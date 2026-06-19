use super::NavigationGraph;
use glam::Vec3;
use std::collections::{BinaryHeap, HashMap};

#[derive(Copy, Clone, PartialEq)]
struct NodeState {
    x: i32,
    z: i32,
    g_score: f32,
    f_score: f32,
}

impl Eq for NodeState {}

impl Ord for NodeState {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Min-heap on f_score (BinaryHeap is a max-heap, so reverse). Break ties on
        // (x, z) with a fixed total order so the popped sequence — and therefore the
        // resulting path — is byte-identical across runs (#130 determinism guard).
        other
            .f_score
            .partial_cmp(&self.f_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(other.x.cmp(&self.x))
            .then(other.z.cmp(&self.z))
    }
}

impl PartialOrd for NodeState {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// The mutable A* working set: the open priority queue plus the best-known
/// g-scores and predecessor links keyed by grid cell.
#[derive(Default)]
struct Frontier {
    open_set: BinaryHeap<NodeState>,
    g_score: HashMap<(i32, i32), f32>,
    came_from: HashMap<(i32, i32), (i32, i32)>,
}

impl Frontier {
    fn g_of(&self, key: (i32, i32)) -> f32 {
        *self.g_score.get(&key).unwrap_or(&f32::INFINITY)
    }
}

impl NavigationGraph {
    /// Queries the next logical step along the shortest path, carrying the real
    /// baked surface height (#130) so the agent follows ramps/stairs in `y`.
    pub fn get_next_path_step(&self, start: Vec3, target: Vec3) -> Vec3 {
        let (sx, sz) = self.world_to_grid(start);
        let (tx, tz) = self.world_to_grid(target);

        if sx == tx && sz == tz {
            return target;
        }

        if let Some(path) = self.find_path(sx, sz, tx, tz) {
            if path.len() > 1 {
                let (nx, nz) = path[1];
                // World position carries the cell's surface height in y.
                return self.grid_to_world(nx, nz);
            } else if path.len() == 1 {
                return target;
            }
        }

        target
    }

    /// Whether an agent may move between two adjacent cells. Both must be explicitly
    /// walkable, and the surface height delta must clear both limits (#130): the
    /// absolute `max_step` (a curb taller than this is a ledge/wall) and the grade
    /// `max_slope` (rise per unit of horizontal travel, so a long diagonal tolerates
    /// a slightly larger rise than a cardinal one). A wall top, raised far above all
    /// neighbours, fails this against every neighbour and is therefore unreachable.
    pub(super) fn step_connected(&self, ax: i32, az: i32, bx: i32, bz: i32) -> bool {
        if !self.is_walkable(ax, az) || !self.is_walkable(bx, bz) {
            return false;
        }
        let dh = (self.height_at(ax, az) - self.height_at(bx, bz)).abs();
        let diagonal = ax != bx && az != bz;
        let run = self.grid_spacing
            * if diagonal {
                std::f32::consts::SQRT_2
            } else {
                1.0
            };
        dh <= self.max_step && dh <= self.max_slope * run
    }

    /// Core A* algorithm returning list of grid coordinates (x, z)
    pub fn find_path(
        &self,
        raw_sx: i32,
        raw_sz: i32,
        raw_tx: i32,
        raw_tz: i32,
    ) -> Option<Vec<(i32, i32)>> {
        let (sx, sz) = self.closest_walkable(raw_sx, raw_sz);
        let (tx, tz) = self.closest_walkable(raw_tx, raw_tz);

        if sx == tx && sz == tz {
            return Some(vec![(sx, sz)]);
        }

        let mut frontier = Frontier::default();
        frontier.g_score.insert((sx, sz), 0.0);
        frontier.open_set.push(NodeState {
            x: sx,
            z: sz,
            g_score: 0.0,
            f_score: self.heuristic(sx, sz, tx, tz),
        });

        while let Some(current) = frontier.open_set.pop() {
            if current.x == tx && current.z == tz {
                return Some(reconstruct_path(
                    &frontier.came_from,
                    (current.x, current.z),
                ));
            }

            let current_g = frontier.g_of((current.x, current.z));
            if current.g_score > current_g {
                continue; // Old state in heap, skip
            }

            self.expand_neighbors(&current, current_g, (tx, tz), &mut frontier);
        }

        None
    }

    /// Relax the 8-way neighbours of `current`, pushing improved nodes onto the
    /// open set and recording the predecessor / g-score for each. Connectivity is
    /// height-aware: a neighbour beyond `max_step` of surface delta is skipped, and
    /// the move cost includes the vertical climb so flatter routes win (#130).
    fn expand_neighbors(
        &self,
        current: &NodeState,
        current_g: f32,
        target: (i32, i32),
        frontier: &mut Frontier,
    ) {
        let (tx, tz) = target;
        let curr_key = (current.x, current.z);

        // Neighbors (8-way movement): (dx, dz, horizontal cost).
        let dirs = [
            (1, 0, 1.0),
            (-1, 0, 1.0),
            (0, 1, 1.0),
            (0, -1, 1.0), // Cardinal
            (1, 1, 1.414),
            (1, -1, 1.414),
            (-1, 1, 1.414),
            (-1, -1, 1.414), // Diagonal
        ];

        for &(dx, dz, horiz) in &dirs {
            let nx = current.x + dx;
            let nz = current.z + dz;

            if !self.step_connected(current.x, current.z, nx, nz) {
                continue;
            }

            // Diagonal: prevent corner-cutting AND require both cardinal cells be
            // step-connected to the current cell (can't slip diagonally past a ledge).
            if dx != 0
                && dz != 0
                && (!self.step_connected(current.x, current.z, current.x + dx, current.z)
                    || !self.step_connected(current.x, current.z, current.x, current.z + dz))
            {
                continue;
            }

            // Cost = horizontal travel + vertical climb, so a ramp costs more than
            // flat ground over the same XZ distance.
            let dh = (self.height_at(nx, nz) - self.height_at(current.x, current.z)).abs();
            let neighbor_key = (nx, nz);
            let tentative_g = current_g + horiz + dh;

            if tentative_g < frontier.g_of(neighbor_key) {
                frontier.came_from.insert(neighbor_key, curr_key);
                frontier.g_score.insert(neighbor_key, tentative_g);
                frontier.open_set.push(NodeState {
                    x: nx,
                    z: nz,
                    g_score: tentative_g,
                    f_score: tentative_g + self.heuristic(nx, nz, tx, tz),
                });
            }
        }
    }

    fn heuristic(&self, ax: i32, az: i32, bx: i32, bz: i32) -> f32 {
        // Octile distance for 8-way grid (admissible lower bound; the vertical climb
        // in g only ever adds cost, so the XZ-only heuristic never overestimates).
        let dx = (ax - bx).abs() as f32;
        let dz = (az - bz).abs() as f32;
        let dmin = dx.min(dz);
        let dmax = dx.max(dz);
        dmin * 1.414 + (dmax - dmin) * 1.0
    }
}

/// Walk the `came_from` chain back from `goal` to the start, returning the path
/// in start->goal order.
fn reconstruct_path(
    came_from: &HashMap<(i32, i32), (i32, i32)>,
    goal: (i32, i32),
) -> Vec<(i32, i32)> {
    let mut path = vec![goal];
    let mut curr_key = goal;
    while let Some(&prev) = came_from.get(&curr_key) {
        path.push(prev);
        curr_key = prev;
    }
    path.reverse();
    path
}
