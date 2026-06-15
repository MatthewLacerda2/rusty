use crate::scene::Scene;
use glam::Vec3;

mod astar;

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

                // Mark grid nodes inside the AABB as blocked
                for gz in 0..self.height {
                    for gx in 0..self.width {
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

    /// Steers and updates positions of active NavMesh agents in the scene,
    /// constraining them strictly to walkable NavMesh cells using a 2D sliding projection check.
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
                    // Query next path node
                    let next_step = self.get_next_path_step(current_pos, agent.target);
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
