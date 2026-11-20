//! Where a camera can be, which decides the cells the occlusion bake samples
//! (#2328). Most of a level's grid is air no camera reaches (under a hangar roof,
//! over a courtyard), so the bake samples only the cells a camera can stand or
//! jump in, within [`PLAY_HEIGHT`] over walkable floor, and the cells holding a
//! shadow-casting light (a light's shadow views read its cell's row,
//! `render::scene::occlusion`), plus the one-cell ring the dilation reads them
//! from. Every other cell sees every object: the PVS culls nothing there, as
//! outside the grid, and Hi-Z still does. Unreal places its precomputed-visibility
//! cells the same way, over surfaces up to a play height.
//!
//! The floor is the navmesh's every walkable span when the scene has one baked,
//! else a floor plan rasterized from the static colliders, else there is none and
//! every cell is sampled, as before ([`FloorSource`]). A pure function of the
//! scene: no clock, no thread order.

use glam::{IVec3, Vec3};

use rusty_components::LightType;
use rusty_scene::lighting::floor::FloorGrid;
use rusty_scene::lighting::occlusion::{FloorSource, Grid};
use rusty_scene::Scene;

/// How far over a walkable floor a camera can be, metres: a standing agent's
/// eye and a jump on top (the navmesh's agent height, 2 m, and a 1 m allowance).
/// A cell whose bottom is under it, over a floor, is sampled. A later authored
/// Occlusion Area (Unity's) is the developer's override.
pub const PLAY_HEIGHT: f32 = 3.0;

/// The floor plan's cell when it is rasterized from the static colliders, metres.
pub const COLLIDER_FLOOR_CELL: f32 = 0.5;

/// Where a camera can be: boxes of standable air over the floor, and the
/// shadow-casting lights. [`Reach::everywhere`] has no floor, and every cell is
/// sampled.
#[derive(Clone, Debug, Default)]
pub struct Reach {
    pub source: FloorSource,
    standable: Vec<(Vec3, Vec3)>,
    lights: Vec<Vec3>,
}

impl Reach {
    /// No floor: every cell is sampled.
    pub fn everywhere() -> Self {
        Self::default()
    }

    /// `scene`'s reach: over `navmesh` (the scene's baked navmesh, every walkable
    /// span) when it has a floor, else over the static colliders' floor plan, else
    /// everywhere.
    pub fn survey(scene: &Scene, navmesh: Option<FloorGrid>) -> Self {
        let colliders = || FloorGrid::from_static(scene, COLLIDER_FLOOR_CELL);
        let found = [(FloorSource::Navmesh, navmesh)].into_iter();
        let mut floors = found.chain(std::iter::once_with(|| {
            (FloorSource::Colliders, colliders())
        }));
        floors
            .find_map(|(source, floor)| {
                let reach = Self::over(scene, &floor?, source);
                (!reach.standable.is_empty()).then_some(reach)
            })
            .unwrap_or_default()
    }

    /// The reach over `floor`, found by `source`, with `scene`'s lights.
    pub fn over(scene: &Scene, floor: &FloorGrid, source: FloorSource) -> Self {
        let half = floor.cell * 0.5;
        let column = |i: usize, y: f32| {
            let c = floor.centre(i % floor.width.max(1), i / floor.width.max(1));
            let lo = Vec3::new(c.x - half, y, c.y - half);
            (lo, Vec3::new(c.x + half, y + PLAY_HEIGHT, c.y + half))
        };
        let lowest = (floor.cells.iter().enumerate())
            .filter_map(|(i, c)| c.map(|c| (i, c.floor)))
            .filter(|(_, y)| y.is_finite());
        let upper = floor.upper.iter().copied().filter(|(_, y)| y.is_finite());
        Self {
            source,
            standable: lowest.chain(upper).map(|(i, y)| column(i, y)).collect(),
            lights: shadow_lights(scene),
        }
    }

    /// The grid over `(min, max)` and, unless every cell is sampled, which of its
    /// cells are: the smallest cell whose sampled count fits (`Grid::fitting`).
    pub fn grid(&self, min: Vec3, max: Vec3) -> Option<(Grid, Option<Vec<bool>>)> {
        if self.source == FloorSource::Everywhere {
            return Grid::over(min, max).map(|g| (g, None));
        }
        let count = |g: &Grid| self.sampled(g).iter().filter(|&&b| b).count();
        let grid = Grid::fitting(min, max, count)?;
        Some((grid, Some(self.sampled(&grid))))
    }

    /// A hash of the cells of `grid` the bake is seeded from, 0 when every cell is
    /// sampled: what a kept PVS must match (`bake::is_current`).
    pub fn key(&self, grid: &Grid) -> u64 {
        let Some(seeds) = self.seeds(grid) else {
            return 0;
        };
        let mut hash = rusty_core::hash::Content::default();
        hash.write(&self.source.code().to_le_bytes());
        for (i, _) in seeds.iter().enumerate().filter(|(_, &s)| s) {
            hash.write(&(i as u32).to_le_bytes());
        }
        hash.finish()
    }

    /// The cells a camera can be in, and the lights' cells, `None` everywhere.
    fn seeds(&self, grid: &Grid) -> Option<Vec<bool>> {
        if self.source == FloorSource::Everywhere {
            return None;
        }
        let mut seeds = vec![false; grid.cell_count()];
        for &(lo, hi) in &self.standable {
            mark(grid, lo, hi, &mut seeds);
        }
        for cell in self.lights.iter().filter_map(|&p| grid.index_of(p)) {
            seeds[cell] = true;
        }
        Some(seeds)
    }

    /// The seeds and the ring the dilation reads them from: every cell sampled.
    fn sampled(&self, grid: &Grid) -> Vec<bool> {
        let Some(seeds) = self.seeds(grid) else {
            return vec![true; grid.cell_count()];
        };
        let mut sampled = seeds.clone();
        for cell in (0..seeds.len()).filter(|&c| seeds[c]) {
            grid.neighbourhood(cell).for_each(|n| sampled[n] = true);
        }
        sampled
    }
}

/// Mark every cell of `grid` the box `(lo, hi)` reaches inside: a cell whose
/// bottom is under `hi.y` and top over `lo.y`.
fn mark(grid: &Grid, lo: Vec3, hi: Vec3, seeds: &mut [bool]) {
    let last = grid.dims.as_ivec3() - IVec3::ONE;
    let from = ((lo - grid.origin) / grid.cell).floor().as_ivec3();
    let to = ((hi - grid.origin) / grid.cell).ceil().as_ivec3() - IVec3::ONE;
    if from.cmpgt(last).any() || to.cmplt(IVec3::ZERO).any() {
        return;
    }
    let (from, to) = (from.max(IVec3::ZERO), to.min(last));
    for y in from.y..=to.y {
        for z in from.z..=to.z {
            for x in from.x..=to.x {
                seeds[grid.index(IVec3::new(x, y, z).as_uvec3())] = true;
            }
        }
    }
}

/// Where `scene`'s active shadow-casting lights are, in entity order: a point,
/// spot or area light whose shadow views read its cell's row.
fn shadow_lights(scene: &Scene) -> Vec<Vec3> {
    let world = &scene.world;
    let casts = |id: &u32| {
        world.is_active_in_hierarchy(*id)
            && world.light(*id).is_some_and(|l| {
                l.cast_shadows
                    && !matches!(l.light_type, LightType::Ambient | LightType::Directional)
            })
    };
    let ids = world.ids_with_light().into_iter().filter(casts);
    ids.map(|id| world.world_position(id)).collect()
}
