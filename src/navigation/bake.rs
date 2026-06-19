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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{ColliderComponent, ColliderShape, Scene};

    /// Adds a static box collider spanning the given world AABB to `scene`.
    pub(super) fn add_box(scene: &mut Scene, name: &str, min: Vec3, max: Vec3) {
        let id = scene.add_entity(name.to_string());
        let mut e = scene.get_entity_mut(id).expect("entity exists");
        e.is_static = true;
        e.collider = Some(ColliderComponent {
            active: true,
            shape: ColliderShape::Box { size: max - min },
            is_trigger: false,
            aabb_min: min,
            aabb_max: max,
        });
    }

    /// A staircase rising 0.5/step at z = 4 (cells x = 3..=6), ground elsewhere.
    fn stair_graph() -> NavigationGraph {
        let mut scene = Scene::new();
        for (i, gx) in (3..=6).enumerate() {
            let y = 0.5 * i as f32;
            let fx = gx as f32;
            add_box(
                &mut scene,
                &format!("step{i}"),
                Vec3::new(fx - 0.25, 0.0, 2.0),
                Vec3::new(fx + 0.25, y, 6.0),
            );
        }
        let mut graph = NavigationGraph::new(0.0, 10.0, 0.0, 10.0, 1.0);
        graph.bake(&scene);
        graph
    }

    #[test]
    fn stairs_bake_rising_height_field() {
        let graph = stair_graph();
        assert_eq!(graph.height_at(3, 4), 0.0);
        assert_eq!(graph.height_at(4, 4), 0.5);
        assert_eq!(graph.height_at(5, 4), 1.0);
        assert_eq!(graph.height_at(6, 4), 1.5);
        // Each 0.5 step is within max_step, so adjacent run cells stay connected.
        for gx in 3..=5 {
            assert!(
                graph.step_connected(gx, 4, gx + 1, 4),
                "step {gx} -> {} connected",
                gx + 1
            );
        }
        // Every step cell is reachable from a neighbour within max_step.
        for gx in 3..=6 {
            assert!(graph.cell_reachable(gx, 4), "step {gx} reachable");
        }
    }

    #[test]
    fn steep_wall_top_is_unreachable() {
        // Tall isolated box (top y=3) surrounded by ground (y=0): every neighbour is
        // a 3.0 drop, far beyond max_step 0.5, so the top is unreachable and no edge
        // connects to it — while the surrounding ground stays walkable.
        let mut scene = Scene::new();
        // Box footprint [3.6,4.4]x[3.6,4.4] covers only the single cell-center (4,4).
        add_box(
            &mut scene,
            "wall",
            Vec3::new(3.6, 0.0, 3.6),
            Vec3::new(4.4, 3.0, 4.4),
        );
        let mut graph = NavigationGraph::new(0.0, 10.0, 0.0, 10.0, 1.0);
        graph.bake(&scene);
        assert_eq!(graph.height_at(4, 4), 3.0);
        assert_eq!(graph.height_at(5, 4), 0.0, "neighbour stays ground");
        assert!(
            !graph.cell_reachable(4, 4),
            "wall top is isolated by 3.0 drops"
        );
        assert!(!graph.step_connected(3, 4, 4, 4), "cannot step up the wall");
        assert!(
            graph.cell_reachable(1, 1),
            "distant flat ground stays reachable"
        );
    }

    #[test]
    fn bake_is_deterministic_across_runs() {
        let a = stair_graph();
        let b = stair_graph();
        assert_eq!(a.heightfield, b.heightfield, "height field must be stable");
        assert_eq!(a.walkability, b.walkability, "walkability must be stable");
        assert_eq!(a.bake_generation, b.bake_generation);
    }

    /// Kill "replace > with <" in raise_surface max-fold: overlapping boxes must
    /// leave the cell at the MAXIMUM top, not the minimum or the first seen.
    #[test]
    fn raise_surface_takes_max_of_overlapping_colliders() {
        let mut scene = Scene::new();
        add_box(
            &mut scene,
            "lo",
            Vec3::new(3.0, 0.0, 3.0),
            Vec3::new(5.0, 1.0, 5.0),
        );
        add_box(
            &mut scene,
            "hi",
            Vec3::new(3.0, 0.0, 3.0),
            Vec3::new(5.0, 3.0, 5.0),
        );
        let mut g = NavigationGraph::new(0.0, 10.0, 0.0, 10.0, 1.0);
        g.bake(&scene);
        assert_eq!(g.height_at(4, 4), 3.0, "max(1.0, 3.0) = 3.0");
        assert_eq!(g.height_at(3, 3), 3.0, "corner cell also takes max");
    }

    /// Kill cell_reachable `<= → <`: a cell exactly max_step above a neighbour
    /// must be reachable; max_step + epsilon must not.
    #[test]
    fn cell_reachable_boundary_at_max_step() {
        let mut g = NavigationGraph::new(0.0, 10.0, 0.0, 10.0, 1.0);
        let idx = g.index(5, 5);
        g.heightfield[idx] = g.max_step;
        assert!(g.cell_reachable(5, 5), "exactly max_step: reachable");
        g.heightfield[idx] += 0.001;
        assert!(!g.cell_reachable(5, 5), "above max_step: unreachable");
    }
}
