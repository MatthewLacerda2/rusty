//! src/navigation/bounds.rs — where the navmesh grid lies in the world (#452).
//!
//! The grid used to be a hardcoded 40×40 box around the origin, so anything past ±20
//! units was unnavigable however the level was built. The bake now resolves its XZ
//! bounds from the scene every time it runs:
//!
//! * an **authored override** (`NavMeshSettings::bounds`, Unity's nav volume) when the
//!   scene carries a valid one;
//! * otherwise the **XZ extent of the static geometry** the bake already reads, grown by
//!   [`BOUNDS_MARGIN`] plus the agent radius so the radius erosion's pull-back off the
//!   world edge never eats the ground around the outermost geometry;
//! * an **empty scene** (no static geometry) falls back to the historical ±20 box
//!   ([`EMPTY_SCENE_HALF_EXTENT`]). With nothing to stand on it holds no spans
//!   (#454): no geometry, no navmesh, as in Unity.
//!
//! Resolved bounds snap outward to multiples of the grid spacing, so cell centres stay
//! on the same world lattice whatever the geometry's extent (a moved crate never shifts
//! every cell by a fraction). Pure AABB math — no RNG/clock — so the determinism guard
//! for `navigation` holds.

use serde::{Deserialize, Serialize};

use super::NavigationGraph;
use crate::scene::Scene;
use glam::Vec3;

/// Ground (world units) kept around the outermost static geometry, on top of the agent
/// radius, when the bounds are derived rather than authored.
pub const BOUNDS_MARGIN: f32 = 2.0;
/// Half-extent of the default box an empty scene (no static geometry) bakes over — the
/// historical hardcoded ±20 bounds.
pub const EMPTY_SCENE_HALF_EXTENT: f32 = 20.0;

/// An XZ rectangle in world units: the area the navmesh grid covers.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct NavBounds {
    pub min_x: f32,
    pub max_x: f32,
    pub min_z: f32,
    pub max_z: f32,
}

impl NavBounds {
    pub const fn new(min_x: f32, max_x: f32, min_z: f32, max_z: f32) -> Self {
        Self {
            min_x,
            max_x,
            min_z,
            max_z,
        }
    }

    /// The default box an empty scene bakes over (±[`EMPTY_SCENE_HALF_EXTENT`]).
    pub const EMPTY_SCENE: Self = Self {
        min_x: -EMPTY_SCENE_HALF_EXTENT,
        max_x: EMPTY_SCENE_HALF_EXTENT,
        min_z: -EMPTY_SCENE_HALF_EXTENT,
        max_z: EMPTY_SCENE_HALF_EXTENT,
    };

    /// Finite, with a positive extent on both axes. The bake ignores an invalid
    /// override (falls back to the derived bounds) so it stays a total function.
    pub fn is_valid(&self) -> bool {
        let finite = [self.min_x, self.max_x, self.min_z, self.max_z]
            .iter()
            .all(|v| v.is_finite());
        finite && self.min_x < self.max_x && self.min_z < self.max_z
    }

    /// Grow every side by `margin`.
    fn grown(self, margin: f32) -> Self {
        Self {
            min_x: self.min_x - margin,
            max_x: self.max_x + margin,
            min_z: self.min_z - margin,
            max_z: self.max_z + margin,
        }
    }

    /// Snap outward to multiples of `spacing`, keeping cell centres on a fixed lattice.
    fn snapped(self, spacing: f32) -> Self {
        Self {
            min_x: (self.min_x / spacing).floor() * spacing,
            max_x: (self.max_x / spacing).ceil() * spacing,
            min_z: (self.min_z / spacing).floor() * spacing,
            max_z: (self.max_z / spacing).ceil() * spacing,
        }
    }
}

/// Every collider the bake reads: entity active, static, collider active and not a
/// trigger (Unity leaves triggers out of the navmesh). The one definition of "static
/// geometry" shared by the rasteriser and the bounds.
pub(super) fn static_collider_ids(scene: &Scene) -> impl Iterator<Item = u32> + '_ {
    scene
        .world
        .ids_with_collider()
        .into_iter()
        .filter(move |&id| {
            scene.world.is_active(id)
                && scene.world.is_static(id)
                && scene
                    .world
                    .collider(id)
                    .is_some_and(|c| c.active && !c.is_trigger)
        })
}

/// World AABBs of the bake's static colliders.
fn static_aabbs(scene: &Scene) -> impl Iterator<Item = (Vec3, Vec3)> + '_ {
    static_collider_ids(scene).filter_map(move |id| {
        let col = scene.world.collider(id)?;
        Some((col.aabb_min, col.aabb_max))
    })
}

/// The XZ extent of the scene's static geometry, `None` when there is none.
fn geometry_extent(scene: &Scene) -> Option<NavBounds> {
    static_aabbs(scene).fold(None, |acc, (min, max)| {
        let b = NavBounds {
            min_x: min.x,
            max_x: max.x,
            min_z: min.z,
            max_z: max.z,
        };
        Some(match acc {
            None => b,
            Some(a) => NavBounds {
                min_x: a.min_x.min(b.min_x),
                max_x: a.max_x.max(b.max_x),
                min_z: a.min_z.min(b.min_z),
                max_z: a.max_z.max(b.max_z),
            },
        })
    })
}

/// The bounds a bake of `scene` at `spacing` covers: the authored override when valid,
/// else the static geometry's extent plus margin, else the empty-scene box — snapped
/// outward to the spacing lattice.
pub fn resolve_bounds(scene: &Scene, spacing: f32) -> NavBounds {
    let s = &scene.nav_settings;
    let raw = match s.bounds.filter(NavBounds::is_valid) {
        Some(authored) => authored,
        None => geometry_extent(scene)
            .map(|g| g.grown(BOUNDS_MARGIN + s.agent_radius.max(0.0)))
            .filter(NavBounds::is_valid)
            .unwrap_or(NavBounds::EMPTY_SCENE),
    };
    raw.snapped(spacing)
}

impl NavigationGraph {
    /// A graph baked from `scene` — the one construction path the windowed game and the
    /// headless harness share, so both navigate the same grid for the same scene.
    pub fn from_scene(scene: &Scene) -> Self {
        let b = NavBounds::EMPTY_SCENE;
        let mut graph = Self::new(
            b.min_x,
            b.max_x,
            b.min_z,
            b.max_z,
            super::grid::DEFAULT_GRID_SPACING,
        );
        graph.bake(scene);
        graph
    }

    /// The XZ area this graph currently covers.
    pub fn bounds(&self) -> NavBounds {
        NavBounds {
            min_x: self.min_x,
            max_x: self.max_x,
            min_z: self.min_z,
            max_z: self.max_z,
        }
    }

    /// Re-shape the grid to the scene's resolved bounds and `nav_settings.grid_spacing`
    /// (#276, #452). Cell counts use the SAME formula as [`NavigationGraph::new`], so a
    /// graph created with these bounds/spacing and one re-shaped to them are identical.
    /// Buffers are reallocated only when the shape changes; the caller's reset + collider
    /// pass re-fills them either way. An out-of-range spacing (≤ 0, non-finite) keeps the
    /// current one, so the bake stays a total, panic-free function.
    pub(super) fn reshape_for(&mut self, scene: &Scene) {
        let spacing = scene.nav_settings.grid_spacing;
        let spacing = if spacing.is_finite() && spacing > 0.0 {
            spacing
        } else {
            self.grid_spacing
        };
        let b = resolve_bounds(scene, spacing);
        if b == self.bounds() && spacing == self.grid_spacing {
            return;
        }
        *self = Self {
            bake_generation: self.bake_generation,
            ..Self::new(b.min_x, b.max_x, b.min_z, b.max_z, spacing)
        };
    }
}
