use super::NavigationGraph;
use crate::scene::Scene;
use glam::Vec3;

impl NavigationGraph {
    /// Re-bakes the surface height field from static colliders (#130).
    ///
    /// Each static collider contributes its world AABB *top* as a walkable surface;
    /// a cell takes the highest top covering it (so stairs and overlapping terrain
    /// keep the upper surface), else flat ground `y = 0`. Walkability between cells
    /// is not a per-cell flag but a connectivity rule applied at query time
    /// (`NavigationGraph::step_connected`): a cell raised far above all its
    /// neighbours (a wall top) is simply unreachable — no neighbour is within
    /// `max_step` — so A\* never routes onto it and steering never slides up it.
    /// This unifies the pathing and steering rules and correctly keeps stair tops
    /// walkable (reachable by a chain of small steps) while rejecting wall faces.
    /// Deterministic: fixed scene iteration order and an order-independent max-fold.
    pub fn bake(&mut self, scene: &Scene) {
        // Source the per-scene bake tunables (#276) instead of the historical
        // hardcoded defaults: step/slope are cheap scalar copies; grid_spacing may
        // re-shape the grid (see `apply_grid_spacing`). `agent_radius` is read into
        // the settings but deliberately UNUSED here — it is stored-but-inert until the
        // radius-erosion follow-up (#277). Doing this first means the reset below sizes
        // the freshly-spaced grid.
        self.max_step = scene.nav_settings.max_step;
        self.max_slope = scene.nav_settings.max_slope;
        self.apply_grid_spacing(scene.nav_settings.grid_spacing);

        // A new bake may change cell heights / reachability, so bump the generation:
        // any agent whose cached path was planned against an older bake re-plans (#126).
        self.bake_generation = self.bake_generation.wrapping_add(1);

        // Reset: flat ground at y = 0, fully walkable. Static colliders raise the
        // surface to their AABB top.
        self.walkability.fill(true);
        self.heightfield.fill(0.0);

        for entity in scene.iter() {
            if !entity.active || !entity.is_static {
                continue;
            }
            if let Some(col) = &entity.collider {
                if col.active {
                    self.raise_surface(col.aabb_min, col.aabb_max);
                }
            }
        }
    }

    /// Re-shape the grid to a new cell size (#276 `grid_spacing` knob) when it differs
    /// from the current spacing, keeping the same world bounds. Cell counts are derived
    /// with the SAME formula as [`NavigationGraph::new`], so a graph created at spacing
    /// `s` and one re-spaced to `s` are identical. The walkability / height buffers are
    /// resized and zeroed here; the caller's reset + collider pass then re-fills them,
    /// so the knob actually changes the baked resolution. A no-op when unchanged (the
    /// common case), so the cheap step/slope path stays cheap. Out-of-range spacings
    /// (≤ 0, non-finite) are ignored to keep the bake a total, panic-free function.
    fn apply_grid_spacing(&mut self, spacing: f32) {
        if !spacing.is_finite() || spacing <= 0.0 || spacing == self.grid_spacing {
            return;
        }
        self.grid_spacing = spacing;
        self.width = ((self.max_x - self.min_x) / spacing).ceil() as i32 + 1;
        self.height = ((self.max_z - self.min_z) / spacing).ceil() as i32 + 1;
        let cells = (self.width * self.height) as usize;
        self.walkability = vec![true; cells];
        self.heightfield = vec![0.0; cells];
    }

    /// Raise the surface height of every cell whose center lies under this
    /// collider's footprint to the collider's AABB top, taking the max so stacked
    /// geometry (stairs) and overlapping terrain keep the highest walkable surface.
    fn raise_surface(&mut self, min: Vec3, max: Vec3) {
        // Only cells under this AABB's footprint can change, so iterate just that
        // sub-rectangle. Map corners through `world_to_grid` (clamped), then widen
        // by one cell so rounding can't drop a boundary cell; the per-cell
        // containment test keeps the result identical to a full-grid scan (#125).
        let (gx_min, gz_min) = self.world_to_grid(Vec3::new(min.x, 0.0, min.z));
        let (gx_max, gz_max) = self.world_to_grid(Vec3::new(max.x, 0.0, max.z));
        let gx0 = (gx_min - 1).max(0);
        let gz0 = (gz_min - 1).max(0);
        let gx1 = (gx_max + 1).min(self.width - 1);
        let gz1 = (gz_max + 1).min(self.height - 1);

        for gz in gz0..=gz1 {
            for gx in gx0..=gx1 {
                let w = self.grid_to_world(gx, gz);
                if w.x >= min.x && w.x <= max.x && w.z >= min.z && w.z <= max.z {
                    let idx = self.index(gx, gz);
                    if max.y > self.heightfield[idx] {
                        self.heightfield[idx] = max.y;
                    }
                }
            }
        }
    }

    /// Whether a cell is *reachable* from any cardinal neighbour within `max_step`
    /// (a wall top, isolated by tall drops on every side, is not). Drives the bake's
    /// notion of an effectively-blocked cell in tests; A\* and steering enforce the
    /// same `max_step` rule per-edge via `NavigationGraph::step_connected`.
    pub fn cell_reachable(&self, gx: i32, gz: i32) -> bool {
        let h = self.height_at(gx, gz);
        [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|&(dx, dz)| {
            let (nx, nz) = (gx + dx, gz + dz);
            if nx < 0 || nx >= self.width || nz < 0 || nz >= self.height {
                return false;
            }
            (self.height_at(nx, nz) - h).abs() <= self.max_step
        })
    }
}
