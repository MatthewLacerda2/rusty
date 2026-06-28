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
        // re-shape the grid (see `apply_grid_spacing`). `agent_radius` is consumed by
        // the erosion pass below (#277). Doing the spacing first means the reset below
        // sizes the freshly-spaced grid.
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

        // Agent-height clearance (#278): carve cells whose vertical clearance to the
        // lowest static collider overhead is below the agent height (no crawling under a
        // low overhang). Runs AFTER `raise_surface` settles the floor (clearance is
        // measured from the final per-cell floor) but BEFORE the radius erosion below, so
        // the radius clearance also pulls back from the freshly-carved holes — a
        // newly-blocked low-headroom region is treated like any other obstacle the agent
        // must keep its footprint off of (headroom-carve THEN radius-erode).
        self.carve_low_headroom(scene);

        // Agent-radius erosion (#277): pull the final walkable surface back off every
        // obstacle by the agent radius. Runs LAST so it erodes the settled surface
        // (heightfield + reachability + the headroom holes carved just above), not an
        // intermediate one.
        self.erode_for_agent_radius(scene.nav_settings.agent_radius);
    }

    /// Erode the walkable surface inward by `agent_radius` (#277), the standard
    /// Recast/Unity meaning: an agent is a disc of this radius, so any walkable cell
    /// whose center is within `agent_radius` of a wall (or the world edge) can't hold the
    /// agent's footprint and is cleared. Passages narrower than ~`2 * agent_radius` close
    /// up, and the surface keeps clearance off the walls instead of hugging them.
    ///
    /// Algorithm (deterministic, pure grid math — no RNG/clock, see the determinism
    /// guard for `navigation`):
    /// * **Radius → cells** by `ceil(agent_radius / grid_spacing)` — the conservative
    ///   rounding: round *up* so a partially-covered cell still erodes (never leave an
    ///   agent clipping a wall). `radius_cells == 0` (which includes `agent_radius == 0`)
    ///   is an exact no-op, reproducing the pre-#277 bake byte-for-byte.
    /// * **What counts as a "wall"** is the boundary of the connected navigable surface,
    ///   not a per-cell height flag. Cells are partitioned into step/slope-connected
    ///   components ([`Self::connected_components`]); two cells in DIFFERENT components are
    ///   cut by a non-traversable transition (a wall face, a ledge taller than `max_step`).
    ///   So a walkable cell is eroded when, within the radius, there is a cell in a
    ///   different component OR the world edge (out-of-bounds). This treats a contiguous
    ///   wall strip and a lone wall-top uniformly, and — crucially — never erodes across a
    ///   stair/ramp, which is one connected component.
    /// * **Distance metric**: Euclidean in cell space — clear a cell when such a boundary
    ///   lies within `dx*dx + dz*dz <= radius_cells*radius_cells`, the faithful model of a
    ///   circular agent.
    /// * **Snapshot**: the component labels are computed from the PRE-erosion surface, so a
    ///   single well-defined erosion by the radius happens — newly-cleared cells never seed
    ///   further erosion in the same pass (no iterated shrink).
    fn erode_for_agent_radius(&mut self, agent_radius: f32) {
        let radius_cells = (agent_radius / self.grid_spacing).ceil() as i32;
        if radius_cells <= 0 {
            return; // agent_radius == 0 (or sub-grid radius rounding to 0): exact no-op.
        }
        let labels = self.connected_components();
        let r2 = radius_cells * radius_cells;
        let mut cleared = vec![false; self.walkability.len()];
        for gz in 0..self.height {
            for gx in 0..self.width {
                let idx = self.index(gx, gz);
                if self.walkability[idx] && self.boundary_within(&labels, gx, gz, radius_cells, r2)
                {
                    cleared[idx] = true;
                }
            }
        }
        for (idx, &c) in cleared.iter().enumerate() {
            if c {
                self.walkability[idx] = false;
            }
        }
    }

    /// True when some cell within Euclidean `radius_cells` of `(gx, gz)` is across a
    /// navigable-surface boundary from it: a cell in a DIFFERENT step-connected component
    /// (a wall), or out of bounds (the world edge). Scans only the `(2r+1)²` window.
    fn boundary_within(
        &self,
        labels: &[i32],
        gx: i32,
        gz: i32,
        radius_cells: i32,
        r2: i32,
    ) -> bool {
        let here = labels[self.index(gx, gz)];
        for dz in -radius_cells..=radius_cells {
            for dx in -radius_cells..=radius_cells {
                if dx * dx + dz * dz > r2 {
                    continue; // outside the agent's circular footprint
                }
                let (nx, nz) = (gx + dx, gz + dz);
                let crossed = if nx < 0 || nx >= self.width || nz < 0 || nz >= self.height {
                    true // out-of-bounds: pull the surface back from the world edge
                } else {
                    labels[self.index(nx, nz)] != here
                };
                if crossed {
                    return true;
                }
            }
        }
        false
    }

    /// Label each cell with its step/slope-connected component id (a deterministic flood
    /// fill in fixed cell order). Cells reachable from one another by traversable steps
    /// (flat ground, a stair run, a ramp) share a label; a wall — cut off by drops beyond
    /// `max_step`/`max_slope` from the surrounding floor — lands in its own component, which
    /// is exactly the boundary the erosion pulls back from. Computed on the pre-erosion
    /// surface, so erosion is a single shrink by the radius, not an iterated one.
    fn connected_components(&self) -> Vec<i32> {
        let mut labels = vec![-1_i32; self.walkability.len()];
        let mut next = 0_i32;
        for gz in 0..self.height {
            for gx in 0..self.width {
                if labels[self.index(gx, gz)] == -1 {
                    self.flood_component(gx, gz, next, &mut labels);
                    next += 1;
                }
            }
        }
        labels
    }

    /// Flood-fill the step-connected component containing `(sx, sz)` with `label`.
    fn flood_component(&self, sx: i32, sz: i32, label: i32, labels: &mut [i32]) {
        let mut stack = vec![(sx, sz)];
        labels[self.index(sx, sz)] = label;
        while let Some((cx, cz)) = stack.pop() {
            for &(dx, dz) in &[(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (nx, nz) = (cx + dx, cz + dz);
                if nx < 0 || nx >= self.width || nz < 0 || nz >= self.height {
                    continue;
                }
                let nidx = self.index(nx, nz);
                if labels[nidx] == -1 && self.step_connected(cx, cz, nx, nz) {
                    labels[nidx] = label;
                    stack.push((nx, nz));
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
