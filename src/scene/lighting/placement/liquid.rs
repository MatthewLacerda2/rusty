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
//! Every floor cell under a liquid's surface is dropped from the plan, so the
//! floor around the pool keeps its own probe and none captures from the water.
//!
//! Pure data + math — deterministic, entity order.

use glam::Vec2;

use super::FloorGrid;
use crate::components::RenderMode;
use crate::scene::Scene;

/// The most a liquid's surface may rise across its bounds, world units.
const FLAT: f32 = 0.05;

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
    /// Drop every floor under one of `liquids`' surfaces (a cell's centre over
    /// its bounds, its floor below the surface), the upper floors too.
    pub fn drown(&mut self, liquids: &[Liquid]) {
        let under = |centre: Vec2, floor: f32| {
            (liquids.iter()).any(|l| {
                floor < l.surface && centre.cmpge(l.min).all() && centre.cmple(l.max).all()
            })
        };
        for i in 0..self.cells.len() {
            let centre = self.centre(i % self.width, i / self.width);
            if self.cells[i].is_some_and(|c| under(centre, c.floor)) {
                self.cells[i] = None;
            }
        }
        let (width, origin, cell) = (self.width, self.origin, self.cell);
        self.upper.retain(|&(i, floor)| {
            let centre = origin + Vec2::new((i % width) as f32, (i / width) as f32) * cell;
            !under(centre, floor)
        });
    }
}
