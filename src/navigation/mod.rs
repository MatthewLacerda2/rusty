use crate::scene::{NavMeshAgentComponent, Scene};
use glam::Vec3;
use std::collections::{BinaryHeap, HashMap};

/// Squared world-distance the target may drift before a cached agent path is
/// considered stale and re-planned. Keeps tiny jitter from re-running A*.
const REPLAN_TARGET_EPSILON_SQ: f32 = 0.25;
/// Upper bound (in fixed-update frames) on how long a cached path may live
/// before a forced re-plan. Frame-count based, never wall-clock, so the sim
/// stays a pure function of (seed, inputs, dt).
const MAX_PATH_AGE_FRAMES: u32 = 60;
/// How close (world units, XZ plane) the agent must get to a waypoint before
/// the cursor advances to the next one.
const WAYPOINT_REACHED_DISTANCE: f32 = 0.5;

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

pub struct NavigationGraph {
    pub min_x: f32,
    pub max_x: f32,
    pub min_z: f32,
    pub max_z: f32,
    pub grid_spacing: f32,
    // Spacing size
    pub width: i32,
    pub height: i32,
    // Walkability grid
    pub walkability: Vec<bool>,
    /// Monotonic counter bumped on every `bake`. Agents stamp the generation
    /// their cached path was planned against (#126); a mismatch forces a re-plan,
    /// so a rebake (e.g. a moved static collider) transparently invalidates every
    /// stale agent path without `bake` needing mutable access to the scene.
    pub bake_generation: u64,
}

impl NavigationGraph {
    pub fn new(min_x: f32, max_x: f32, min_z: f32, max_z: f32, spacing: f32) -> Self {
        let width = ((max_x - min_x) / spacing).ceil() as i32 + 1;
        let height = ((max_z - min_z) / spacing).ceil() as i32 + 1;
        let walkability = vec![true; (width * height) as usize];

        Self {
            min_x,
            max_x,
            min_z,
            max_z,
            grid_spacing: spacing,
            width,
            height,
            walkability,
            bake_generation: 0,
        }
    }

    /// Converts a world coordinate to grid coordinates (x, z)
    pub fn world_to_grid(&self, pos: Vec3) -> (i32, i32) {
        let x = ((pos.x - self.min_x) / self.grid_spacing).round() as i32;
        let z = ((pos.z - self.min_z) / self.grid_spacing).round() as i32;
        (x.clamp(0, self.width - 1), z.clamp(0, self.height - 1))
    }

    /// Converts grid coordinates to world coordinate (x, 0, z)
    pub fn grid_to_world(&self, gx: i32, gz: i32) -> Vec3 {
        Vec3::new(
            self.min_x + (gx as f32) * self.grid_spacing,
            0.0,
            self.min_z + (gz as f32) * self.grid_spacing,
        )
    }

    fn index(&self, gx: i32, gz: i32) -> usize {
        (gz * self.width + gx) as usize
    }

    /// Re-bakes the grid walkability using the Scene's AABBs
    pub fn bake(&mut self, scene: &Scene) {
        // A new bake may change which cells are walkable, so bump the generation:
        // any agent whose cached path was planned against an older bake re-plans.
        self.bake_generation = self.bake_generation.wrapping_add(1);

        // Reset walkability
        self.walkability.fill(true);

        for entity in scene.iter() {
            // Only static colliders block pathfinding
            if !entity.active || !entity.is_static {
                continue;
            }

            if let Some(col) = &entity.collider {
                if !col.active {
                    continue;
                }

                // Check bounds of this entity's AABB
                let min = col.aabb_min;
                let max = col.aabb_max;

                // Pad obstacle bounds slightly for enemy size buffer
                let pad = 0.5;
                let obstacle_min_x = min.x - pad;
                let obstacle_max_x = max.x + pad;
                let obstacle_min_z = min.z - pad;
                let obstacle_max_z = max.z + pad;

                // Only the cells under this collider's padded AABB can flip to
                // blocked, so iterate just that sub-rectangle instead of the whole
                // grid. Map the corners through `world_to_grid` (clamped), then
                // widen by one cell so its rounding can't drop a boundary cell.
                // The per-cell containment test below keeps the result byte-identical
                // to a full-grid scan — we only skip cells we know would fail it.
                let (gx_min, gz_min) =
                    self.world_to_grid(Vec3::new(obstacle_min_x, 0.0, obstacle_min_z));
                let (gx_max, gz_max) =
                    self.world_to_grid(Vec3::new(obstacle_max_x, 0.0, obstacle_max_z));
                let gx0 = (gx_min - 1).max(0);
                let gz0 = (gz_min - 1).max(0);
                let gx1 = (gx_max + 1).min(self.width - 1);
                let gz1 = (gz_max + 1).min(self.height - 1);

                // Mark grid nodes inside the AABB as blocked
                for gz in gz0..=gz1 {
                    for gx in gx0..=gx1 {
                        let w_pos = self.grid_to_world(gx, gz);
                        if w_pos.x >= obstacle_min_x
                            && w_pos.x <= obstacle_max_x
                            && w_pos.z >= obstacle_min_z
                            && w_pos.z <= obstacle_max_z
                        {
                            let idx = self.index(gx, gz);
                            self.walkability[idx] = false;
                        }
                    }
                }
            }
        }
    }

    pub fn is_walkable(&self, gx: i32, gz: i32) -> bool {
        if gx < 0 || gx >= self.width || gz < 0 || gz >= self.height {
            return false;
        }
        self.walkability[self.index(gx, gz)]
    }

    /// Finds the closest walkable grid node in concentric rings up to radius 5 if the original node is blocked.
    pub fn closest_walkable(&self, gx: i32, gz: i32) -> (i32, i32) {
        if self.is_walkable(gx, gz) {
            return (gx, gz);
        }

        let mut best_dist = f32::MAX;
        let mut best_node = (gx, gz);

        // Search concentric square rings up to radius 5
        for r in 1_i32..=5_i32 {
            let mut found = false;
            for dx in -r..=r {
                for dz in -r..=r {
                    // Check only the border of the ring to prioritize closest distance
                    if dx.abs() != r && dz.abs() != r {
                        continue;
                    }
                    let nx = gx + dx;
                    let nz = gz + dz;
                    if self.is_walkable(nx, nz) {
                        let dist_sq = (dx * dx + dz * dz) as f32;
                        if dist_sq < best_dist {
                            best_dist = dist_sq;
                            best_node = (nx, nz);
                            found = true;
                        }
                    }
                }
            }
            if found {
                break;
            }
        }
        best_node
    }

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
    #[allow(clippy::too_many_lines)] // grandfathered: burn down in #124
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

        let mut open_set = BinaryHeap::new();
        let mut g_score = HashMap::new();
        let mut came_from = HashMap::new();

        let start_key = (sx, sz);
        g_score.insert(start_key, 0.0);

        open_set.push(NodeState {
            x: sx,
            z: sz,
            g_score: 0.0,
            f_score: self.heuristic(sx, sz, tx, tz),
        });

        while let Some(current) = open_set.pop() {
            if current.x == tx && current.z == tz {
                // Reconstruct path
                let mut path = vec![(current.x, current.z)];
                let mut curr_key = (current.x, current.z);
                while let Some(&prev) = came_from.get(&curr_key) {
                    path.push(prev);
                    curr_key = prev;
                }
                path.reverse();
                return Some(path);
            }

            let curr_key = (current.x, current.z);
            let current_g = *g_score.get(&curr_key).unwrap_or(&f32::INFINITY);

            if current.g_score > current_g {
                continue; // Old state in heap, skip
            }

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
                let neighbor_g = *g_score.get(&neighbor_key).unwrap_or(&f32::INFINITY);

                if tentative_g < neighbor_g {
                    came_from.insert(neighbor_key, curr_key);
                    g_score.insert(neighbor_key, tentative_g);
                    open_set.push(NodeState {
                        x: nx,
                        z: nz,
                        g_score: tentative_g,
                        f_score: tentative_g + self.heuristic(nx, nz, tx, tz),
                    });
                }
            }
        }

        None
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

    /// Whether `agent`'s cached path must be discarded and re-planned. Re-plan
    /// when there is no path, when the cursor has run off the end, when the
    /// navmesh was rebaked since planning, when the target drifted past the
    /// epsilon, when the path has aged out, or when the next waypoint is no
    /// longer walkable.
    fn path_cache_invalid(&self, agent: &NavMeshAgentComponent) -> bool {
        if agent.cached_path.is_empty() || agent.path_cursor >= agent.cached_path.len() {
            return true;
        }
        if agent.path_generation != self.bake_generation {
            return true;
        }
        if (agent.target - agent.planned_target).length_squared() > REPLAN_TARGET_EPSILON_SQ {
            return true;
        }
        if agent.frames_since_replan >= MAX_PATH_AGE_FRAMES {
            return true;
        }
        let wp = agent.cached_path[agent.path_cursor];
        let (gx, gz) = self.world_to_grid(wp);
        !self.is_walkable(gx, gz)
    }

    /// Runs A* once from the agent's current cell to its target and stores the
    /// resulting waypoints on the agent, resetting the cursor and bookkeeping.
    /// The start cell is dropped (the agent is already there) and the literal
    /// target is appended so the agent finishes on the goal, not the goal's cell
    /// center. A failed search still yields a one-waypoint beeline to the target.
    fn plan_agent_path(&self, agent: &mut NavMeshAgentComponent, current_pos: Vec3) {
        let (sx, sz) = self.world_to_grid(current_pos);
        let (tx, tz) = self.world_to_grid(agent.target);
        let mut waypoints = Vec::new();
        if let Some(path) = self.find_path(sx, sz, tx, tz) {
            for &(gx, gz) in path.iter().skip(1) {
                waypoints.push(self.grid_to_world(gx, gz));
            }
        }
        waypoints.push(agent.target);
        agent.cached_path = waypoints;
        agent.path_cursor = 0;
        agent.planned_target = agent.target;
        agent.path_generation = self.bake_generation;
        agent.frames_since_replan = 0;
    }

    /// Returns the world position the agent should steer toward this frame,
    /// using the cached path (re-planning only when invalid) and advancing the
    /// cursor past any waypoints already reached. Y is taken from the agent so
    /// horizontal steering matches the legacy `get_next_path_step` behaviour.
    fn cached_next_step(&self, agent: &mut NavMeshAgentComponent, current_pos: Vec3) -> Vec3 {
        if self.path_cache_invalid(agent) {
            self.plan_agent_path(agent, current_pos);
        }
        while agent.path_cursor < agent.cached_path.len() {
            let wp = agent.cached_path[agent.path_cursor];
            let dx = wp.x - current_pos.x;
            let dz = wp.z - current_pos.z;
            if (dx * dx + dz * dz).sqrt() <= WAYPOINT_REACHED_DISTANCE {
                agent.path_cursor += 1;
            } else {
                break;
            }
        }
        agent.frames_since_replan = agent.frames_since_replan.saturating_add(1);
        match agent.cached_path.get(agent.path_cursor) {
            Some(wp) => Vec3::new(wp.x, current_pos.y, wp.z),
            None => agent.target,
        }
    }

    /// Steers and updates positions of active NavMesh agents in the scene,
    /// constraining them strictly to walkable NavMesh cells using a 2D sliding projection check.
    #[allow(clippy::too_many_lines)] // grandfathered: burn down in #124
    pub fn tick_nav_agents(&self, scene: &mut Scene, delta_time: f32) {
        for id in scene.entity_ids() {
            let mut entity_guard = match scene.get_entity_mut(id) {
                Some(e) => e,
                None => continue,
            };
            let entity: &mut crate::scene::Entity = &mut entity_guard;
            if !entity.active {
                continue;
            }

            if let Some(agent) = &mut entity.nav_agent {
                if !agent.active {
                    continue;
                }

                let current_pos = entity.transform.position;
                let to_target = agent.target - current_pos;
                let dist = to_target.length();

                if dist > agent.stopping_distance {
                    // Query the next waypoint from the agent's cached path,
                    // re-planning with A* only when the cache is invalid (#126).
                    let next_step = self.cached_next_step(agent, current_pos);
                    let to_next = next_step - current_pos;
                    let to_next_dir = to_next.normalize_or_zero();

                    // Accelerate steering velocity
                    let desired_vel = to_next_dir * agent.speed;
                    let diff_vel = desired_vel - agent.velocity;
                    agent.velocity += diff_vel * (agent.acceleration * delta_time).min(1.0);

                    // Proposed step
                    let proposed_pos_x =
                        current_pos + Vec3::new(agent.velocity.x * delta_time, 0.0, 0.0);
                    let mut final_pos = current_pos;

                    // Test X movement slide
                    let (gx, gz) = self.world_to_grid(proposed_pos_x);
                    if self.is_walkable(gx, gz) {
                        final_pos.x = proposed_pos_x.x;
                    } else {
                        agent.velocity.x = 0.0;
                    }

                    // Test Z movement slide
                    let proposed_pos_z =
                        final_pos + Vec3::new(0.0, 0.0, agent.velocity.z * delta_time);
                    let (gx, gz) = self.world_to_grid(proposed_pos_z);
                    if self.is_walkable(gx, gz) {
                        final_pos.z = proposed_pos_z.z;
                    } else {
                        agent.velocity.z = 0.0;
                    }

                    // Preserve original Y position
                    final_pos.y = current_pos.y;

                    // Apply the constrained position to entity transform
                    entity.transform.position = final_pos;
                } else {
                    // Decelerate to zero velocity when close
                    agent.velocity -= agent.velocity * (agent.acceleration * delta_time).min(1.0);
                    if agent.velocity.length_squared() < 0.001 {
                        agent.velocity = Vec3::ZERO;
                    }
                }

                // Keep entity's collider bounds aligned with transform positioning
                entity.update_collider(None);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{ColliderComponent, ColliderShape, Scene};

    /// Builds an 11x11 grid (0..=10 on each axis, spacing 1.0) with a single
    /// static box collider spanning world [3,5]x[3,5]. With the 0.5 pad the
    /// blocked footprint is exactly the 3x3 block of cells {3,4,5}x{3,4,5}.
    fn baked_scene() -> NavigationGraph {
        let mut scene = Scene::new();
        let id = scene.add_entity("obstacle".to_string());
        {
            let mut e = scene.get_entity_mut(id).expect("entity exists");
            e.is_static = true;
            e.collider = Some(ColliderComponent {
                active: true,
                shape: ColliderShape::Box {
                    size: Vec3::new(2.0, 1.0, 2.0),
                },
                is_trigger: false,
                aabb_min: Vec3::new(3.0, 0.0, 3.0),
                aabb_max: Vec3::new(5.0, 0.0, 5.0),
            });
        }
        let mut graph = NavigationGraph::new(0.0, 10.0, 0.0, 10.0, 1.0);
        graph.bake(&scene);
        graph
    }

    #[test]
    fn bake_blocks_exactly_the_obstacle_footprint() {
        let graph = baked_scene();

        // Every cell in {3,4,5}x{3,4,5} is blocked; nothing outside is.
        let blocked: Vec<(i32, i32)> = (0..graph.height)
            .flat_map(|gz| (0..graph.width).map(move |gx| (gx, gz)))
            .filter(|&(gx, gz)| !graph.is_walkable(gx, gz))
            .collect();

        let mut expected: Vec<(i32, i32)> = Vec::new();
        for gz in 3..=5 {
            for gx in 3..=5 {
                expected.push((gx, gz));
            }
        }
        assert_eq!(blocked, expected, "only the padded AABB footprint blocks");
    }

    #[test]
    fn bake_leaves_cells_just_outside_the_footprint_walkable() {
        let graph = baked_scene();
        // One cell outside the padded AABB on each side stays walkable.
        assert!(graph.is_walkable(2, 4));
        assert!(graph.is_walkable(6, 4));
        assert!(graph.is_walkable(4, 2));
        assert!(graph.is_walkable(4, 6));
        // Interior of the obstacle is blocked.
        assert!(!graph.is_walkable(4, 4));
    }

    // --- #126: cached agent paths ---

    fn open_graph() -> NavigationGraph {
        // 21x21 fully-walkable grid (no colliders baked).
        NavigationGraph::new(0.0, 20.0, 0.0, 20.0, 1.0)
    }

    fn test_agent(target: Vec3) -> NavMeshAgentComponent {
        NavMeshAgentComponent {
            active: true,
            radius: 0.5,
            target,
            speed: 5.0,
            acceleration: 10.0,
            stopping_distance: 0.5,
            velocity: Vec3::ZERO,
            ..Default::default()
        }
    }

    #[test]
    fn fresh_agent_plans_then_reuses_until_target_moves() {
        let graph = open_graph();
        let mut agent = test_agent(Vec3::new(10.0, 0.0, 10.0));
        let start = Vec3::new(1.0, 0.0, 1.0);

        // No path yet → invalid → plan it.
        assert!(graph.path_cache_invalid(&agent));
        graph.plan_agent_path(&mut agent, start);
        assert!(!agent.cached_path.is_empty());
        assert!(!graph.path_cache_invalid(&agent), "fresh plan is reusable");

        // Sub-epsilon jitter does NOT trigger a re-plan.
        agent.target += Vec3::new(0.1, 0.0, 0.0);
        assert!(!graph.path_cache_invalid(&agent));

        // A meaningful target move past the epsilon invalidates the cache.
        agent.target += Vec3::new(3.0, 0.0, 0.0);
        assert!(graph.path_cache_invalid(&agent));
    }

    #[test]
    fn rebake_invalidates_cached_path() {
        let mut graph = open_graph();
        let mut agent = test_agent(Vec3::new(10.0, 0.0, 10.0));
        graph.plan_agent_path(&mut agent, Vec3::new(1.0, 0.0, 1.0));
        assert!(!graph.path_cache_invalid(&agent));

        // A rebake bumps the generation; the agent's path is now stale.
        graph.bake(&Scene::new());
        assert!(graph.path_cache_invalid(&agent));
    }

    #[test]
    fn cached_path_ages_out_after_max_frames() {
        let graph = open_graph();
        let mut agent = test_agent(Vec3::new(10.0, 0.0, 10.0));
        graph.plan_agent_path(&mut agent, Vec3::new(1.0, 0.0, 1.0));
        assert!(!graph.path_cache_invalid(&agent));

        agent.frames_since_replan = MAX_PATH_AGE_FRAMES;
        assert!(graph.path_cache_invalid(&agent));
    }

    #[test]
    fn agent_reaches_goal_through_repeated_ticks() {
        let mut scene = Scene::new();
        let id = scene.add_entity("agent".to_string());
        {
            let mut e = scene.get_entity_mut(id).expect("entity exists");
            e.transform.position = Vec3::new(1.0, 0.0, 1.0);
            e.nav_agent = Some(test_agent(Vec3::new(10.0, 0.0, 10.0)));
        }
        let graph = open_graph();
        let dt = 1.0 / 60.0;
        for _ in 0..2000 {
            graph.tick_nav_agents(&mut scene, dt);
        }

        let pos = scene
            .get_entity(id)
            .expect("entity exists")
            .transform
            .position;
        let dx = pos.x - 10.0;
        let dz = pos.z - 10.0;
        let dist = (dx * dx + dz * dz).sqrt();
        assert!(dist <= 0.6, "agent should arrive near the goal, got {dist}");
    }

    #[test]
    fn tick_results_are_deterministic_across_runs() {
        let run = || {
            let mut scene = Scene::new();
            let id = scene.add_entity("agent".to_string());
            {
                let mut e = scene.get_entity_mut(id).expect("entity exists");
                e.transform.position = Vec3::new(1.0, 0.0, 1.0);
                e.nav_agent = Some(test_agent(Vec3::new(15.0, 0.0, 8.0)));
            }
            let graph = open_graph();
            let dt = 1.0 / 60.0;
            for _ in 0..300 {
                graph.tick_nav_agents(&mut scene, dt);
            }
            let pos = scene
                .get_entity(id)
                .expect("entity exists")
                .transform
                .position;
            pos.to_array()
        };
        assert_eq!(run(), run(), "cached pathing must be deterministic");
    }
}
