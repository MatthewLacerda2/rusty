use glam::Vec3;

pub use rusty_scene::nav_settings::{DEFAULT_GRID_SPACING, DEFAULT_MAX_SLOPE, DEFAULT_MAX_STEP};

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

/// The compact-heightfield half of Recast; no polygon mesh. The spans of cell `i` are
/// `spans[cell_start[i]..cell_start[i + 1]]`, bottom-up. A wall top is a span too, but no
/// neighbour is within a step of it, so A\* never reaches it.
pub struct NavigationGraph {
    pub min_x: f32,
    pub max_x: f32,
    pub min_z: f32,
    pub max_z: f32,
    pub grid_spacing: f32,
    pub width: i32,
    pub height: i32,
    pub cell_start: Vec<u32>,
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
        let width = ((max_x - min_x) / spacing).ceil() as i32 + 1;
        let height = ((max_z - min_z) / spacing).ceil() as i32 + 1;
        let cells = (width * height) as usize;
        let flat = NavSpan {
            y: 0.0,
            ceiling: f32::INFINITY,
            area: super::WALKABLE_AREA,
            surface: super::Surface::FLAT,
        };
        Self {
            min_x,
            max_x,
            min_z,
            max_z,
            grid_spacing: spacing,
            width,
            height,
            cell_start: (0..=cells as u32).collect(),
            spans: vec![flat; cells],
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

    pub(super) fn index(&self, gx: i32, gz: i32) -> usize {
        (gz * self.width + gx) as usize
    }

    pub fn span_range(&self, gx: i32, gz: i32) -> std::ops::Range<usize> {
        if !self.in_bounds(gx, gz) {
            return 0..0;
        }
        let i = self.index(gx, gz);
        self.cell_start[i] as usize..self.cell_start[i + 1] as usize
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_is_one_flat_open_span_per_cell() {
        let g = NavigationGraph::new(0.0, 10.0, 0.0, 10.0, 1.0);
        assert_eq!((g.width, g.height), (11, 11));
        assert_eq!(g.spans.len(), 121);
        assert_eq!(g.cell_start.len(), 122);
        assert!(g
            .spans
            .iter()
            .all(|s| s.y == 0.0 && s.ceiling.is_infinite()));
        assert_eq!(g.spans_at(3, 2).len(), 1);
        assert!(g.spans_at(-1, 0).is_empty(), "out of bounds holds nothing");
    }

    #[test]
    fn span_world_carries_the_floor_height() {
        let mut g = NavigationGraph::new(0.0, 10.0, 0.0, 10.0, 1.0);
        let r = g.span_range(4, 5);
        g.spans[r.start].y = 2.5;
        let s = SpanRef {
            gx: 4,
            gz: 5,
            index: r.start as u32,
        };
        assert_eq!(g.span_world(s), Vec3::new(4.0, 2.5, 5.0));
    }

    #[test]
    fn world_to_grid_rounds_to_nearest_cell() {
        let g = NavigationGraph::new(0.0, 10.0, 0.0, 10.0, 1.0);
        assert_eq!(g.world_to_grid(Vec3::new(3.0, 0.0, 5.0)), (3, 5));
        assert_eq!(g.world_to_grid(Vec3::new(3.6, 0.0, 5.4)), (4, 5));
        assert_eq!(g.world_to_grid(Vec3::new(3.4, 0.0, 4.6)), (3, 5));
        let g2 = NavigationGraph::new(2.0, 12.0, 2.0, 12.0, 1.0);
        assert_eq!(g2.world_to_grid(Vec3::new(5.0, 0.0, 7.0)), (3, 5));
    }

    #[test]
    fn index_is_row_major() {
        let g = NavigationGraph::new(0.0, 10.0, 0.0, 10.0, 1.0);
        assert_eq!(g.index(0, 0), 0);
        assert_eq!(g.index(1, 0), 1);
        assert_eq!(g.index(0, 1), 11);
        assert_eq!(g.index(3, 2), 25);
    }

    #[test]
    fn span_refs_walks_every_span_with_its_cell() {
        let g = NavigationGraph::new(0.0, 2.0, 0.0, 1.0, 1.0);
        let refs: Vec<_> = g.span_refs().collect();
        assert_eq!(refs.len(), 6);
        assert_eq!((refs[4].gx, refs[4].gz, refs[4].index), (1, 1, 4));
    }
}
