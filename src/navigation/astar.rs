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
        other
            .f_score
            .partial_cmp(&self.f_score)
            .unwrap_or(std::cmp::Ordering::Equal)
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
    /// Queries the next logical step along the shortest path
    pub fn get_next_path_step(&self, start: Vec3, target: Vec3) -> Vec3 {
        let (sx, sz) = self.world_to_grid(start);
        let (tx, tz) = self.world_to_grid(target);

        // If starting node is same as target node, just head to target
        if sx == tx && sz == tz {
            return target;
        }

        // Run A*
        if let Some(path) = self.find_path(sx, sz, tx, tz) {
            if path.len() > 1 {
                // Return world position of next step
                let (nx, nz) = path[1];
                let next_node_world = self.grid_to_world(nx, nz);
                // Keep the target Y or height constant to match original start
                return Vec3::new(next_node_world.x, start.y, next_node_world.z);
            } else if path.len() == 1 {
                // Already at the closest walkable node to target, direct move to target
                return target;
            }
        }

        // Default: directly interpolate towards target
        target
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
    /// open set and recording the predecessor / g-score for each.
    fn expand_neighbors(
        &self,
        current: &NodeState,
        current_g: f32,
        target: (i32, i32),
        frontier: &mut Frontier,
    ) {
        let (tx, tz) = target;
        let curr_key = (current.x, current.z);

        // Neighbors (8-way movement)
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

        for &(dx, dz, cost) in &dirs {
            let nx = current.x + dx;
            let nz = current.z + dz;

            if !self.is_walkable(nx, nz) {
                continue;
            }

            // For diagonal movements, prevent corner-cutting through blocked cardinal obstacles
            if dx != 0
                && dz != 0
                && (!self.is_walkable(current.x + dx, current.z)
                    || !self.is_walkable(current.x, current.z + dz))
            {
                continue;
            }

            let neighbor_key = (nx, nz);
            let tentative_g = current_g + cost;

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
        // Octile distance for 8-way grid
        let dx = (ax - bx).abs() as f32;
        let dz = (az - bz).abs() as f32;
        let dmin = dx.min(dz);
        let dmax = dx.max(dz);
        // Cost: 1.0 cardinal, 1.414 diagonal
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
