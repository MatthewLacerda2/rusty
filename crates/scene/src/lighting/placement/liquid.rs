//! Liquids are not floor (#1737). A pool's floor is walkable (an actor wades or
//! swims there), but a reflection probe captured from eye height over it sits at
//! or under the water's surface and reflects the inside of the pit.
//!
//! The engine has no liquid component, so a liquid is what a level builds one
//! from, as the default scene's pool does: a **static, active mesh, flat (under
//! [`FLAT`] tall), drawn `Transparent`, with no solid collider** (none, an
//! inactive one, or a trigger), so nothing stands on it. A collider-less glass
//! pane laid flat reads the same way; give a skylight a collider.
//!
//! The floor at the bottom of a liquid is dropped from the plan (the first one
//! under its surface, when at least [`DEPTH`] down), so the floor around the pool
//! keeps its own probe and none captures from the water. Floors further down stay.
//!
//! Pure data + math — deterministic, entity order.

use glam::Vec2;

use super::FloorGrid;
use crate::Scene;
use rusty_components::RenderMode;

/// The most a liquid's surface may rise across its bounds, world units.
const FLAT: f32 = 0.05;
/// How far under a liquid's surface a floor must be to be its bottom, world units:
/// shallower is a puddle or a decal over the floor, which an actor stands on.
pub const DEPTH: f32 = 0.3;

/// A liquid's surface: its XZ bounds and its height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Liquid {
    pub min: Vec2,
    pub max: Vec2,
    pub surface: f32,
}

/// The liquids of `scene`, by the module docs' rule, in entity order.
pub fn liquids(scene: &Scene) -> Vec<Liquid> {
    let world = &scene.world;
    let mut out = Vec::new();
    for id in world.ids_with_mesh() {
        if !world.is_static(id) || !world.is_active_in_hierarchy(id) {
            continue;
        }
        let solid = world
            .collider(id)
            .is_some_and(|c| c.active && !c.is_trigger);
        let transparent = (world.material(id))
            .and_then(|m| scene.materials.get(m.first()))
            .is_some_and(|m| m.render_mode == RenderMode::Transparent);
        if solid || !transparent {
            continue;
        }
        let mesh = world.mesh(id).expect("id came from ids_with_mesh");
        let Some((lo, hi)) = mesh.world_aabb(world.local_to_world(id)) else {
            continue;
        };
        if hi.y - lo.y <= FLAT && (hi - lo).is_finite() {
            out.push(Liquid {
                min: Vec2::new(lo.x, lo.z),
                max: Vec2::new(hi.x, hi.z),
                surface: hi.y,
            });
        }
    }
    out
}

impl FloorGrid {
    /// Drop the floor under each of `liquids`' surfaces: in every cell whose centre
    /// is over a liquid's bounds, the first floor below the surface (the pit's or
    /// pool's bottom), when it is at least `DEPTH` below. A floor further down (the
    /// ground storey under a rooftop pool) and one barely under (the deck under a
    /// puddle or a grime decal) stay.
    pub fn drown(&mut self, liquids: &[Liquid]) {
        let mut drowned: Vec<(usize, f32)> = Vec::new();
        for i in 0..self.cells.len() {
            let centre = self.centre(i % self.width, i / self.width);
            let over = |l: &&Liquid| centre.cmpge(l.min).all() && centre.cmple(l.max).all();
            for l in liquids.iter().filter(over) {
                let first = self
                    .floors_of(i)
                    .filter(|&f| f < l.surface)
                    .reduce(f32::max);
                if let Some(f) = first.filter(|&f| l.surface - f >= DEPTH) {
                    drowned.push((i, f));
                }
            }
        }
        for &(i, f) in &drowned {
            if self.cells[i].is_some_and(|c| c.floor == f) {
                self.cells[i] = None;
            }
        }
        drowned.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
        self.upper.retain(|&(i, f)| {
            let at = |d: &(usize, f32)| d.0.cmp(&i).then(d.1.total_cmp(&f));
            drowned.binary_search_by(at).is_err()
        });
    }

    /// Every floor of cell `i`: its lowest, then its upper storeys.
    fn floors_of(&self, i: usize) -> impl Iterator<Item = f32> + '_ {
        let from = self.upper.partition_point(|u| u.0 < i);
        let upper = self.upper[from..].iter().take_while(move |u| u.0 == i);
        (self.cells[i].map(|c| c.floor).into_iter()).chain(upper.map(|u| u.1))
    }
}
