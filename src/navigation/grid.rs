use glam::Vec3;

/// Default maximum height an agent can step up/down between two adjacent cells
/// while still treating them as connected (stairs, curbs). World units.
pub const DEFAULT_MAX_STEP: f32 = 0.5;
/// Default maximum walkable slope, expressed as a height delta per cell of
/// horizontal travel (i.e. `rise / grid_spacing`). A ramp steeper than this is
/// not traversable. `1.0` ≈ 45° at unit spacing.
pub const DEFAULT_MAX_SLOPE: f32 = 1.0;

/// Height-field navigation model (#130). The flat 2D walkability grid is extended
/// with a parallel per-cell surface-height field so agents traverse ramps, stairs,
/// and multi-level terrain following real `y`.
///
/// # Representation decision
/// A **per-cell height field** was chosen over a triangle navmesh or layered cells:
/// it is the minimal, deterministic extension of the existing XZ grid — same
/// indexing, same A\* topology, same bake-footprint optimization, same waypoint
/// cache — and it has no floating-point set ordering to threaten byte-identical
/// replay. Each cell stores one surface height (the highest static-collider top
/// covering it, else ground `0.0`). Traversal between adjacent cells is a
/// connectivity rule, not a per-cell flag: a move is allowed only when the surface
/// delta clears both `max_step` (absolute curb height) and `max_slope` (grade,
/// rise per unit of horizontal travel). A wall top — raised far above all its
/// neighbours — fails this against every neighbour and is therefore unreachable,
/// so A\* never routes onto it and steering never slides up it.
///
/// Deliberately scoped out (documented follow-up): true multi-layer overpasses
/// (two walkable surfaces at the same XZ, e.g. a bridge over a path). A single
/// height per cell picks the upper surface there.
pub struct NavigationGraph {
    pub min_x: f32,
    pub max_x: f32,
    pub min_z: f32,
    pub max_z: f32,
    pub grid_spacing: f32,
    pub width: i32,
    pub height: i32,
    /// Per-cell explicit walkability, row-major `gz * width + gx`. Reserved for
    /// hard blocking; reachability is otherwise the per-edge step/slope rule.
    pub walkability: Vec<bool>,
    /// Per-cell surface height (world `y`), parallel to `walkability`.
    pub heightfield: Vec<f32>,
    /// Max absolute step height between adjacent cells for a move to be allowed.
    pub max_step: f32,
    /// Max walkable grade (rise per unit of horizontal travel).
    pub max_slope: f32,
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
        let cells = (width * height) as usize;

        Self {
            min_x,
            max_x,
            min_z,
            max_z,
            grid_spacing: spacing,
            width,
            height,
            walkability: vec![true; cells],
            heightfield: vec![0.0; cells],
            max_step: DEFAULT_MAX_STEP,
            max_slope: DEFAULT_MAX_SLOPE,
            bake_generation: 0,
        }
    }

    /// Converts a world coordinate to grid coordinates (x, z). `y` is discarded
    /// here on purpose — height is recovered separately via [`Self::height_at`].
    pub fn world_to_grid(&self, pos: Vec3) -> (i32, i32) {
        let x = ((pos.x - self.min_x) / self.grid_spacing).round() as i32;
        let z = ((pos.z - self.min_z) / self.grid_spacing).round() as i32;
        (x.clamp(0, self.width - 1), z.clamp(0, self.height - 1))
    }

    /// Converts grid coordinates to a world position on the baked surface: the XZ
    /// cell center carried up to its surface height (#130), no longer hardcoded 0.
    pub fn grid_to_world(&self, gx: i32, gz: i32) -> Vec3 {
        Vec3::new(
            self.min_x + (gx as f32) * self.grid_spacing,
            self.height_at(gx, gz),
            self.min_z + (gz as f32) * self.grid_spacing,
        )
    }

    /// Surface height (world `y`) of a cell; `0.0` for out-of-bounds cells.
    pub fn height_at(&self, gx: i32, gz: i32) -> f32 {
        if gx < 0 || gx >= self.width || gz < 0 || gz >= self.height {
            return 0.0;
        }
        self.heightfield[self.index(gx, gz)]
    }

    pub(super) fn index(&self, gx: i32, gz: i32) -> usize {
        (gz * self.width + gx) as usize
    }

    pub fn is_walkable(&self, gx: i32, gz: i32) -> bool {
        if gx < 0 || gx >= self.width || gz < 0 || gz >= self.height {
            return false;
        }
        self.walkability[self.index(gx, gz)]
    }

    /// Finds the closest walkable grid node in concentric rings up to radius 5 if
    /// the original node is blocked.
    pub fn closest_walkable(&self, gx: i32, gz: i32) -> (i32, i32) {
        if self.is_walkable(gx, gz) {
            return (gx, gz);
        }
        let mut best_dist = f32::MAX;
        let mut best_node = (gx, gz);
        for r in 1_i32..=5_i32 {
            let mut found = false;
            for dx in -r..=r {
                for dz in -r..=r {
                    if dx.abs() != r && dz.abs() != r {
                        continue;
                    }
                    let (nx, nz) = (gx + dx, gz + dz);
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

    #[test]
    fn new_sizes_grid_and_defaults_flat() {
        let g = NavigationGraph::new(0.0, 10.0, 0.0, 10.0, 1.0);
        assert_eq!((g.width, g.height), (11, 11));
        assert_eq!(g.walkability.len(), 121);
        assert_eq!(g.heightfield.len(), 121);
        // A fresh graph is flat ground at y = 0, fully walkable.
        assert!(g.heightfield.iter().all(|&h| h == 0.0));
        assert!(g.walkability.iter().all(|&w| w));
    }

    #[test]
    fn grid_to_world_carries_surface_height() {
        let mut g = NavigationGraph::new(0.0, 10.0, 0.0, 10.0, 1.0);
        let idx = g.index(4, 5);
        g.heightfield[idx] = 2.5;
        let w = g.grid_to_world(4, 5);
        assert_eq!(w, Vec3::new(4.0, 2.5, 5.0));
        // Out-of-bounds height falls back to 0.0.
        assert_eq!(g.height_at(-1, 0), 0.0);
    }

    #[test]
    fn closest_walkable_finds_nearest_open_cell() {
        let mut g = NavigationGraph::new(0.0, 10.0, 0.0, 10.0, 1.0);
        let idx = g.index(5, 5);
        g.walkability[idx] = false;
        let (nx, nz) = g.closest_walkable(5, 5);
        assert!(g.is_walkable(nx, nz));
        assert_eq!(
            (nx - 5).abs() + (nz - 5).abs(),
            1,
            "nearest ring is distance 1"
        );
    }
}
