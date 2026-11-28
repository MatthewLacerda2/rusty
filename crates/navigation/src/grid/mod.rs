use glam::Vec3;

mod frame;
#[cfg(test)]
mod tests;
mod tiles;

pub(crate) use tiles::TileGrid;
pub use tiles::TileKey;

pub use rusty_scene::nav_settings::{
    DEFAULT_GRID_SPACING, DEFAULT_MAX_SLOPE, DEFAULT_MAX_STEP, DEFAULT_TILE_SIZE,
};

/// `ceiling` is `f32::INFINITY` when the sky is open. Recast's compact-heightfield span.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NavSpan {
    pub y: f32,
    pub ceiling: f32,
    pub area: u8,
    /// `y` stays the cell's highest point, which connectivity compares.
    pub surface: super::Surface,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SpanRef {
    pub gx: i32,
    pub gz: i32,
    pub index: u32,
}

/// Recast's compact heightfield: a cell's spans are `spans[span_range(..)]`, bottom-up, stored
/// per tile; `spans` holds dead ids too.
pub struct NavigationGraph {
    pub min_x: f32,
    pub max_x: f32,
    pub min_z: f32,
    pub max_z: f32,
    pub grid_spacing: f32,
    pub width: i32,
    pub height: i32,
    pub(crate) tiles: TileGrid,
    pub spans: Vec<NavSpan>,
    pub max_step: f32,
    /// Not part of the step rule (`links.rs`).
    pub max_slope: f32,
    pub agent_height: f32,
    /// Agents stamp the generation their path was planned against; a mismatch forces a re-plan.
    pub bake_generation: u64,
    pub area_costs: [f32; super::MAX_AREAS],
    pub(super) bake_state: Option<Box<super::bake::BakeState>>,
    pub(super) offmesh: super::offmesh::OffMeshLinks,
    pub(super) static_watch: Option<super::bake::StaticWatch>,
    pub(super) tracked_scene: Option<(rusty_scene::SceneId, super::Start)>,
    pub(super) search: super::pool::SearchScratch,
    pub(super) flood_cache: super::flood::FloodCache,
    pub(super) requests: std::cell::RefCell<super::request::PathQueue>,
    pub(super) islands: super::request::islands::Islands,
}

impl NavigationGraph {
    pub fn new(min_x: f32, max_x: f32, min_z: f32, max_z: f32, spacing: f32) -> Self {
        let side = tile_side(DEFAULT_TILE_SIZE, spacing);
        let mut g = Self::empty(min_x, max_x, min_z, max_z, spacing, side);
        let flat = NavSpan {
            y: 0.0,
            ceiling: f32::INFINITY,
            area: super::WALKABLE_AREA,
            surface: super::Surface::FLAT,
        };
        let all = crate::CellRect::all(&g);
        let mut cols = crate::bake::Columns::with_capacity(all.cells());
        (0..all.cells()).for_each(|_| cols.push_column([flat]));
        g.splice(all, &cols);
        g
    }

    pub(crate) fn empty(
        min_x: f32,
        max_x: f32,
        min_z: f32,
        max_z: f32,
        spacing: f32,
        side: i32,
    ) -> Self {
        let width = ((max_x - min_x) / spacing).ceil() as i32 + 1;
        let height = ((max_z - min_z) / spacing).ceil() as i32 + 1;
        let origin = (
            (min_x / spacing).round() as i32,
            (min_z / spacing).round() as i32,
        );
        Self {
            min_x,
            max_x,
            min_z,
            max_z,
            grid_spacing: spacing,
            width,
            height,
            tiles: TileGrid::new(origin, width, height, side),
            spans: Vec::new(),
            max_step: DEFAULT_MAX_STEP,
            max_slope: DEFAULT_MAX_SLOPE,
            agent_height: super::DEFAULT_AGENT_HEIGHT,
            bake_generation: 0,
            area_costs: [super::MIN_AREA_COST; super::MAX_AREAS],
            bake_state: None,
            offmesh: Default::default(),
            static_watch: None,
            tracked_scene: None,
            search: Default::default(),
            flood_cache: Default::default(),
            requests: Default::default(),
            islands: Default::default(),
        }
    }

    pub fn tile_side(&self) -> i32 {
        self.tiles.side
    }

    pub fn tile_keys(&self) -> Vec<TileKey> {
        self.tiles.keys()
    }

    pub fn mesh_bytes(&self) -> usize {
        self.spans.len() * std::mem::size_of::<NavSpan>() + self.tiles.bytes()
    }

    pub fn tile_count(&self) -> usize {
        self.tiles.count()
    }

    pub fn tile_of(&self, gx: i32, gz: i32) -> TileKey {
        self.tiles.key_of(gx, gz)
    }

    /// The live spans; `spans.len()` counts dead ids too.
    pub fn span_count(&self) -> usize {
        self.spans.len() - self.tiles.dead as usize
    }

    /// `y` is discarded: which span of the column is meant is `snap.rs`'s job.
    pub fn world_to_grid(&self, pos: Vec3) -> (i32, i32) {
        let x = ((pos.x - self.min_x) / self.grid_spacing).round() as i32;
        let z = ((pos.z - self.min_z) / self.grid_spacing).round() as i32;
        (x.clamp(0, self.width - 1), z.clamp(0, self.height - 1))
    }

    pub fn cell_center(&self, gx: i32, gz: i32) -> Vec3 {
        Vec3::new(
            self.min_x + (gx as f32) * self.grid_spacing,
            0.0,
            self.min_z + (gz as f32) * self.grid_spacing,
        )
    }

    pub fn span_world(&self, s: SpanRef) -> Vec3 {
        let mut w = self.cell_center(s.gx, s.gz);
        w.y = self.spans[s.index as usize].y;
        w
    }

    pub fn in_bounds(&self, gx: i32, gz: i32) -> bool {
        gx >= 0 && gx < self.width && gz >= 0 && gz < self.height
    }

    pub(crate) fn index(&self, gx: i32, gz: i32) -> usize {
        (gz * self.width + gx) as usize
    }

    pub fn span_range(&self, gx: i32, gz: i32) -> std::ops::Range<usize> {
        if !self.in_bounds(gx, gz) {
            return 0..0;
        }
        self.tiles.range(gx, gz)
    }

    pub fn spans_at(&self, gx: i32, gz: i32) -> &[NavSpan] {
        &self.spans[self.span_range(gx, gz)]
    }

    /// A test hook: `test_support::ground` asserts with it.
    #[doc(hidden)]
    pub fn is_walkable(&self, gx: i32, gz: i32) -> bool {
        !self.span_range(gx, gz).is_empty()
    }

    pub fn span_refs(&self) -> impl Iterator<Item = SpanRef> + '_ {
        (0..self.height).flat_map(move |gz| {
            (0..self.width).flat_map(move |gx| {
                self.span_range(gx, gz).map(move |i| SpanRef {
                    gx,
                    gz,
                    index: i as u32,
                })
            })
        })
    }
}

/// 16 cells at least, as in Unity.
pub fn tile_side(tile_size: f32, spacing: f32) -> i32 {
    let size = match tile_size.is_finite() && tile_size > 0.0 {
        true => tile_size,
        false => DEFAULT_TILE_SIZE,
    };
    let side = (size / spacing).round();
    match side.is_finite() {
        true => side.clamp(16.0, 16_384.0) as i32,
        false => 128,
    }
}
