//! Derived bounds grow by [`BOUNDS_MARGIN`] plus the agent radius, so the erosion's pull-back never
//! eats the ground around the outermost geometry. Bounds snap outward to multiples of the grid
//! spacing, so a moved crate never shifts every cell by a fraction.

use super::object_modifier::bake_as;
use super::NavigationGraph;
use glam::{Vec2, Vec3};
use rusty_scene::lighting::floor::{FloorCell, FloorGrid};
pub use rusty_scene::nav_settings::NavBounds;
use rusty_scene::Scene;

/// World units, on top of the agent radius, when the bounds are derived.
pub const BOUNDS_MARGIN: f32 = 2.0;

fn grown(b: NavBounds, margin: f32) -> NavBounds {
    NavBounds {
        min_x: b.min_x - margin,
        max_x: b.max_x + margin,
        min_z: b.min_z - margin,
        max_z: b.max_z + margin,
    }
}

fn snapped(b: NavBounds, spacing: f32) -> NavBounds {
    NavBounds {
        min_x: (b.min_x / spacing).floor() * spacing,
        max_x: (b.max_x / spacing).ceil() * spacing,
        min_z: (b.min_z / spacing).floor() * spacing,
        max_z: (b.max_z / spacing).ceil() * spacing,
    }
}

pub fn bake_collider_ids(scene: &Scene) -> Vec<u32> {
    let mut ids: Vec<u32> = static_collider_ids(scene).collect();
    ids.sort_unstable();
    ids
}

/// Unity leaves triggers out of the navmesh.
pub(super) fn static_collider_ids(scene: &Scene) -> impl Iterator<Item = u32> + '_ {
    scene
        .world
        .ids_with_collider()
        .into_iter()
        .filter(move |&id| {
            scene.world.is_active_in_hierarchy(id)
                && scene.world.is_static(id)
                && scene
                    .world
                    .collider(id)
                    .is_some_and(|c| c.active && !c.is_trigger)
                && !bake_as(&scene.world, id).ignored
        })
}

fn static_aabbs(scene: &Scene) -> impl Iterator<Item = (Vec3, Vec3)> + '_ {
    static_collider_ids(scene).filter_map(move |id| scene.world.collider_bounds(id))
}

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

pub fn resolve_bounds(scene: &Scene, spacing: f32) -> NavBounds {
    let s = &scene.nav_settings;
    let raw = match s.bounds.filter(NavBounds::is_valid) {
        Some(authored) => authored,
        None => geometry_extent(scene)
            .map(|g| grown(g, BOUNDS_MARGIN + s.agent_radius.max(0.0)))
            .filter(NavBounds::is_valid)
            .unwrap_or(NavBounds::EMPTY_SCENE),
    };
    snapped(raw, spacing)
}

impl NavigationGraph {
    pub fn from_scene(scene: &Scene) -> Self {
        let b = NavBounds::EMPTY_SCENE;
        let spacing = super::grid::DEFAULT_GRID_SPACING;
        let side = super::grid::tile_side(super::grid::DEFAULT_TILE_SIZE, spacing);
        let mut graph = Self::empty(b.min_x, b.max_x, b.min_z, b.max_z, spacing, side);
        graph.restore_or_bake(scene);
        graph
    }

    pub fn walkable_aabb(&self) -> Option<(Vec3, Vec3)> {
        let baked =
            self.width > 1 && self.height > 1 && self.max_x > self.min_x && self.max_z > self.min_z;
        if !baked {
            return None;
        }
        let (mut min, mut max) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for s in self.span_refs() {
            let w = self.span_world(s);
            min = min.min(w);
            max = max.max(w);
        }
        (min.x <= max.x).then_some((min, max))
    }

    /// The spans stop `agent_radius` short of the walls, which the plan's `inset` carries.
    pub fn floor_grid(&self, agent_radius: f32, reachable: Option<&[bool]>) -> Option<FloorGrid> {
        let baked =
            self.width > 1 && self.height > 1 && self.max_x > self.min_x && self.max_z > self.min_z;
        if !baked {
            return None;
        }
        let counts = |i: &usize| reachable.is_none_or(|r| r.get(*i).copied().unwrap_or(false));
        let mut cells = Vec::with_capacity((self.width * self.height) as usize);
        let mut upper = Vec::new();
        for (gx, gz) in (0..self.height).flat_map(|gz| (0..self.width).map(move |gx| (gx, gz))) {
            let mut floors = self.span_range(gx, gz).filter(counts);
            cells.push(floors.next().map(|lowest| FloorCell {
                floor: self.spans[lowest].y,
                ceiling: self.spans[lowest].ceiling,
            }));
            let cell = cells.len() - 1;
            upper.extend(floors.map(|i| (cell, self.spans[i].y)));
        }
        Some(FloorGrid {
            origin: Vec2::new(self.min_x, self.min_z),
            cell: self.grid_spacing,
            width: self.width as usize,
            depth: self.height as usize,
            cells,
            upper,
            inset: agent_radius.max(0.0),
        })
    }

    pub fn bounds(&self) -> NavBounds {
        NavBounds {
            min_x: self.min_x,
            max_x: self.max_x,
            min_z: self.min_z,
            max_z: self.max_z,
        }
    }

    /// An out-of-range spacing keeps the current one, so the bake stays total.
    pub(super) fn reshape_for(&mut self, scene: &Scene) {
        let shape = self.target_shape(scene);
        if shape == self.shape_now() {
            return;
        }
        let (b, spacing, side) = shape;
        *self = Self {
            bake_generation: self.bake_generation,
            tracked_scene: self.tracked_scene.clone(),
            static_watch: self.static_watch.take(),
            requests: std::mem::take(&mut self.requests),
            ..Self::empty(b.min_x, b.max_x, b.min_z, b.max_z, spacing, side)
        };
    }

    pub(super) fn needs_reshape(&self, scene: &Scene) -> bool {
        self.target_shape(scene) != self.shape_now()
    }

    pub(super) fn shape_now(&self) -> (NavBounds, f32, i32) {
        (self.bounds(), self.grid_spacing, self.tiles.side)
    }

    pub(super) fn target_shape(&self, scene: &Scene) -> (NavBounds, f32, i32) {
        let spacing = scene.nav_settings.grid_spacing;
        let spacing = if spacing.is_finite() && spacing > 0.0 {
            spacing
        } else {
            self.grid_spacing
        };
        let side = super::grid::tile_side(scene.nav_settings.tile_size, spacing);
        (resolve_bounds(scene, spacing), spacing, side)
    }
}
