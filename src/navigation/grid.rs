use crate::scene::Scene;
use glam::Vec3;

/// The walkable-grid model: the `NavigationGraph` struct, world<->grid coordinate
/// conversions, and the bake that derives walkability from static colliders. The
/// search ([`super::astar`]) and the agent tick ([`super::agents`]) build on this.
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
}
