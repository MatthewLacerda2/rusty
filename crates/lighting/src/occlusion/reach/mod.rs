use glam::{IVec3, Vec3};

use rusty_components::LightType;
use rusty_scene::lighting::floor::FloorGrid;
use rusty_scene::lighting::occlusion::{FloorSource, Grid};
use rusty_scene::nav_settings::NavMeshSettings;
use rusty_scene::Scene;

pub const JUMP_ALLOWANCE: f32 = 1.0;

pub const MIN_PLAY_HEIGHT: f32 = 3.0;

pub fn play_height(settings: &NavMeshSettings) -> f32 {
    let jump = settings.jump_height.max(JUMP_ALLOWANCE);
    let height = settings.agent_height + jump;
    match height.is_finite() {
        true => height.max(MIN_PLAY_HEIGHT),
        false => MIN_PLAY_HEIGHT,
    }
}

#[derive(Clone, Debug, Default)]
pub struct Reach {
    pub source: FloorSource,
    pub play_height: f32,
    standable: Vec<(Vec3, Vec3)>,
    points: Vec<Vec3>,
    walkable: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Cells {
    pub grid: Grid,
    pub source: FloorSource,
    pub sampled: Option<Vec<bool>>,
    pub key: u64,
}

impl Cells {
    pub fn sampled_count(&self) -> usize {
        self.sampled
            .as_ref()
            .map_or(self.grid.cell_count(), |s| s.iter().filter(|&&b| b).count())
    }
}

struct Marks {
    standable: Vec<bool>,
    seeds: Vec<bool>,
}

impl Reach {
    pub fn everywhere() -> Self {
        Self::default()
    }

    pub fn over(scene: &Scene, floor: &FloorGrid) -> Self {
        let height = play_height(&scene.nav_settings);
        let half = floor.cell * 0.5 + floor.inset.max(0.0);
        let column = |i: usize, y: f32| {
            let c = floor.centre(i % floor.width.max(1), i / floor.width.max(1));
            let lo = Vec3::new(c.x - half, y, c.y - half);
            (lo, Vec3::new(c.x + half, y + height, c.y + half))
        };
        let lowest = (floor.cells.iter().enumerate())
            .filter_map(|(i, c)| c.map(|c| (i, c.floor)))
            .filter(|(_, y)| y.is_finite());
        let upper = floor.upper.iter().copied().filter(|(_, y)| y.is_finite());
        let standable: Vec<_> = lowest.chain(upper).map(|(i, y)| column(i, y)).collect();
        let columns = floor.cells.iter().filter(|c| c.is_some()).count();
        let mut points = cameras(scene);
        points.extend(shadow_lights(scene));
        Self {
            source: FloorSource::Navmesh,
            play_height: height,
            standable,
            points,
            walkable: columns as f32 * floor.cell * floor.cell,
        }
    }

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

    pub fn standable(&self, grid: &Grid) -> Option<Vec<bool>> {
        (self.source != FloorSource::Everywhere)
            .then(|| self.marks(grid).standable)
            .filter(|s| s.contains(&true))
    }

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

    fn key(&self, seeds: &[bool]) -> u64 {
        let mut hash = rusty_core::hash::Content::default();
        hash.write(&self.source.code().to_le_bytes());
        hash.write(&self.play_height.to_bits().to_le_bytes());
        for (i, _) in seeds.iter().enumerate().filter(|(_, &s)| s) {
            hash.write(&(i as u32).to_le_bytes());
        }
        hash.finish()
    }

    fn marks(&self, grid: &Grid) -> Marks {
        let mut standable = vec![false; grid.cell_count()];
        for &(lo, hi) in &self.standable {
            mark(grid, lo, hi, &mut standable);
        }
        let mut seeds = standable.clone();
        for cell in self.points.iter().filter_map(|&p| grid.index_of(p)) {
            seeds[cell] = true;
        }
        Marks { standable, seeds }
    }
}

fn ring(grid: &Grid, seeds: &[bool]) -> Vec<bool> {
    let mut sampled = seeds.to_vec();
    for cell in (0..seeds.len()).filter(|&c| seeds[c]) {
        grid.neighbourhood(cell).for_each(|n| sampled[n] = true);
    }
    sampled
}

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

fn cameras(scene: &Scene) -> Vec<Vec3> {
    let world = &scene.world;
    let ids = world.ids_with_camera().into_iter();
    ids.map(|id| world.world_position(id)).collect()
}

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
mod seeds_tests;
#[cfg(test)]
mod tests;
