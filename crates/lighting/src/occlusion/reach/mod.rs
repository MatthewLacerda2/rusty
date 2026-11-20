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
//! The floor is the navmesh's every walkable span, handed over as a
//! `FloorGrid` when the scene's navmesh is baked and current; with none, every
//! cell is sampled, as before ([`FloorSource`]). No floor is guessed from the
//! colliders: a guess that misses a storey or an interior would cull less there
//! than a full bake. A navmesh that marks no cell of the grid falls back to every
//! cell too. A pure function of the scene: no clock, no thread order.

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

/// Where a camera can be: boxes of standable air over the floor, and the
/// shadow-casting lights. [`Reach::everywhere`] has no floor, and every cell is
/// sampled.
#[derive(Clone, Debug, Default)]
pub struct Reach {
    pub source: FloorSource,
    standable: Vec<(Vec3, Vec3)>,
    lights: Vec<Vec3>,
    /// The floor's footprint, square metres: its columns that hold any floor.
    walkable: f32,
}

/// The grid a bake lays and which of its cells it samples ([`Reach::grid`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Cells {
    pub grid: Grid,
    /// Where the floor came from: [`FloorSource::Everywhere`] also when the reach
    /// had a floor that marked no cell of this grid.
    pub source: FloorSource,
    /// Per cell whether it is sampled; `None` when every cell is.
    pub sampled: Option<Vec<bool>>,
    /// A hash of the cells it is seeded from, 0 when every cell is sampled: what a
    /// kept PVS must match (`bake::is_current`).
    pub key: u64,
}

impl Cells {
    /// How many cells are sampled.
    pub fn sampled_count(&self) -> usize {
        self.sampled
            .as_ref()
            .map_or(self.grid.cell_count(), |s| s.iter().filter(|&&b| b).count())
    }
}

/// One grid's marks: the cells a camera can be in, and with the lights' the seeds.
struct Marks {
    standable: Vec<bool>,
    seeds: Vec<bool>,
}

impl Reach {
    /// No floor: every cell is sampled.
    pub fn everywhere() -> Self {
        Self::default()
    }

    /// The reach over `floor`, the navmesh's walkable spans (every one: each
    /// column's lowest and the floors above it), with `scene`'s shadow lights.
    pub fn over(scene: &Scene, floor: &FloorGrid) -> Self {
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
        let standable: Vec<_> = lowest.chain(upper).map(|(i, y)| column(i, y)).collect();
        let columns = floor.cells.iter().filter(|c| c.is_some()).count();
        Self {
            source: FloorSource::Navmesh,
            standable,
            lights: shadow_lights(scene),
            walkable: columns as f32 * floor.cell * floor.cell,
        }
    }

    /// The grid over `(min, max)` and which of its cells are sampled: the smallest
    /// cell whose sampled count fits (`Grid::fitting`), each candidate marked once.
    /// A floor that marks no cell a camera can be in samples every cell, as
    /// [`Self::everywhere`] does.
    pub fn grid(&self, min: Vec3, max: Vec3) -> Option<Cells> {
        let everywhere = || {
            Grid::over(min, max).map(|grid| Cells {
                grid,
                source: FloorSource::Everywhere,
                sampled: None,
                key: 0,
            })
        };
        if self.source == FloorSource::Everywhere {
            return everywhere();
        }
        let (grid, marks) = Grid::fitting(min, max, |g| {
            let marks = self.marks(g);
            let sampled = ring(g, &marks.seeds);
            (sampled.iter().filter(|&&b| b).count(), (marks, sampled))
        })?;
        let (marks, sampled) = marks;
        if !marks.standable.contains(&true) {
            return everywhere();
        }
        Some(Cells {
            key: self.key(&marks.seeds),
            grid,
            source: self.source,
            sampled: Some(sampled),
        })
    }

    /// Per cell of `grid` whether a camera can be in it (its row is exact), `None`
    /// when every cell is sampled (every row is). What the bake report's
    /// visibility figures are taken over.
    pub fn standable(&self, grid: &Grid) -> Option<Vec<bool>> {
        (self.source != FloorSource::Everywhere)
            .then(|| self.marks(grid).standable)
            .filter(|s| s.contains(&true))
    }

    /// The share of the footprint of `(min, max)` (the static bounds) the floor
    /// covers, 0 to 1; `None` with no floor. A stale or partial navmesh shows as a
    /// small share in the bake report.
    pub fn coverage(&self, (min, max): (Vec3, Vec3)) -> Option<f32> {
        if self.source == FloorSource::Everywhere {
            return None;
        }
        let footprint = (max.x - min.x).max(0.0) * (max.z - min.z).max(0.0);
        Some(match footprint > 0.0 {
            true => (self.walkable / footprint).clamp(0.0, 1.0),
            false => 1.0,
        })
    }

    /// A hash of the floor's source and the seed cells: one pass over them.
    fn key(&self, seeds: &[bool]) -> u64 {
        let mut hash = rusty_core::hash::Content::default();
        hash.write(&self.source.code().to_le_bytes());
        for (i, _) in seeds.iter().enumerate().filter(|(_, &s)| s) {
            hash.write(&(i as u32).to_le_bytes());
        }
        hash.finish()
    }

    /// The cells of `grid` a camera can be in, and with the lights' cells the
    /// seeds the bake samples round.
    fn marks(&self, grid: &Grid) -> Marks {
        let mut standable = vec![false; grid.cell_count()];
        for &(lo, hi) in &self.standable {
            mark(grid, lo, hi, &mut standable);
        }
        let mut seeds = standable.clone();
        for cell in self.lights.iter().filter_map(|&p| grid.index_of(p)) {
            seeds[cell] = true;
        }
        Marks { standable, seeds }
    }
}

/// The `seeds` and the ring the dilation reads them from.
fn ring(grid: &Grid, seeds: &[bool]) -> Vec<bool> {
    let mut sampled = seeds.to_vec();
    for cell in (0..seeds.len()).filter(|&c| seeds[c]) {
        grid.neighbourhood(cell).for_each(|n| sampled[n] = true);
    }
    sampled
}

/// Mark every cell of `grid` the box `(lo, hi)` reaches inside: a cell whose
/// bottom is under `hi.y` and top over `lo.y`.
fn mark(grid: &Grid, lo: Vec3, hi: Vec3, cells: &mut [bool]) {
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
                cells[grid.index(IVec3::new(x, y, z).as_uvec3())] = true;
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

#[cfg(test)]
mod dust2_tests;
#[cfg(test)]
mod kept_tests;
#[cfg(test)]
mod tests;
