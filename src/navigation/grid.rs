use glam::Vec3;

/// Default maximum height an agent can step up/down between two adjacent cells
/// while still treating them as connected (stairs, curbs). World units.
pub const DEFAULT_MAX_STEP: f32 = 0.5;
/// Default maximum walkable slope, expressed as a height delta per cell of
/// horizontal travel (i.e. `rise / grid_spacing`). A ramp steeper than this is
/// not traversable. `1.0` ≈ 45° at unit spacing.
pub const DEFAULT_MAX_SLOPE: f32 = 1.0;
/// Default grid cell size in world units — the spacing every runtime
/// `NavigationGraph` is created with. The per-scene [`NavMeshSettings`] default
/// matches this so an unconfigured scene bakes at the historical resolution.
///
/// [`NavMeshSettings`]: super::NavMeshSettings
pub const DEFAULT_GRID_SPACING: f32 = 1.0;

/// One walkable surface in a cell's column: a floor an agent can stand on and the
/// open space above it, up to the next solid (`ceiling`, `f32::INFINITY` when the
/// sky is open). Recast's compact-heightfield span.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NavSpan {
    /// World `y` of the floor.
    pub y: f32,
    /// World `y` of the underside of the next solid above, or `f32::INFINITY`.
    pub ceiling: f32,
}

/// A walkable span by position: its cell and its index into [`NavigationGraph::spans`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SpanRef {
    pub gx: i32,
    pub gz: i32,
    pub index: u32,
}

/// Layered navigation grid (#454): an XZ grid whose every cell holds an ordered
/// list of walkable spans, so stacked floors, a catwalk over a corridor or a bridge
/// over a road are all walkable at once. This is the compact-heightfield half of
/// Recast; there is no polygon mesh on top of it.
///
/// # Storage
/// A flat compact array: [`Self::spans`] holds every walkable span, grouped by cell
/// in row-major order and sorted bottom-up within a cell; the spans of cell `i` are
/// `spans[cell_start[i]..cell_start[i + 1]]`. One allocation for the whole level,
/// no per-cell `Vec` headers, and a span's index is a stable A\* node id.
///
/// # Traversal
/// A move between spans in adjacent cells is a connectivity rule, not a stored edge
/// (`links.rs`): the floor delta must clear `max_step` and `max_slope`, and the open
/// gap the two spans share must fit `agent_height`. A wall top is a span too, but
/// no neighbour is within a step of it, so A\* never reaches it.
pub struct NavigationGraph {
    pub min_x: f32,
    pub max_x: f32,
    pub min_z: f32,
    pub max_z: f32,
    pub grid_spacing: f32,
    pub width: i32,
    pub height: i32,
    /// Per-cell span ranges into [`Self::spans`], `width * height + 1` prefix offsets.
    pub cell_start: Vec<u32>,
    /// Every walkable span, grouped by cell (row-major), bottom-up within a cell.
    pub spans: Vec<NavSpan>,
    /// Max absolute step height between adjacent spans for a move to be allowed.
    pub max_step: f32,
    /// Max walkable grade (rise per unit of horizontal travel).
    pub max_slope: f32,
    /// Minimum open height a span (and a move between two spans) must offer.
    pub agent_height: f32,
    /// Monotonic counter bumped on every `bake`. Agents stamp the generation
    /// their cached path was planned against (#126); a mismatch forces a re-plan,
    /// so a rebake (e.g. a moved static collider) transparently invalidates every
    /// stale agent path without `bake` needing mutable access to the scene.
    pub bake_generation: u64,
    /// What the last bake kept to rebake incrementally (#456); `None` until the
    /// first bake.
    pub(super) bake_state: Option<Box<super::bake::BakeState>>,
    /// Off-mesh links (#462): generated drops and jumps, and authored links.
    pub(super) offmesh: super::offmesh::OffMeshLinks,
}

impl NavigationGraph {
    /// An unbaked graph: one open span on flat ground at `y = 0` in every cell. The
    /// first [`Self::bake`] replaces it with what the scene's geometry supports, so
    /// this flat floor exists only for graphs that are never baked (tests, tools).
    pub fn new(min_x: f32, max_x: f32, min_z: f32, max_z: f32, spacing: f32) -> Self {
        let width = ((max_x - min_x) / spacing).ceil() as i32 + 1;
        let height = ((max_z - min_z) / spacing).ceil() as i32 + 1;
        let cells = (width * height) as usize;
        let flat = NavSpan {
            y: 0.0,
            ceiling: f32::INFINITY,
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
            bake_state: None,
            offmesh: Default::default(),
        }
    }

    /// Converts a world coordinate to grid coordinates (x, z), clamped to the grid.
    /// `y` is discarded: which span of the column is meant is `snap.rs`'s job.
    pub fn world_to_grid(&self, pos: Vec3) -> (i32, i32) {
        let x = ((pos.x - self.min_x) / self.grid_spacing).round() as i32;
        let z = ((pos.z - self.min_z) / self.grid_spacing).round() as i32;
        (x.clamp(0, self.width - 1), z.clamp(0, self.height - 1))
    }

    /// World XZ of a cell's centre (`y = 0`).
    pub fn cell_center(&self, gx: i32, gz: i32) -> Vec3 {
        Vec3::new(
            self.min_x + (gx as f32) * self.grid_spacing,
            0.0,
            self.min_z + (gz as f32) * self.grid_spacing,
        )
    }

    /// World position of a span: its cell's centre carried up to its floor.
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

    /// The index range of a cell's spans in [`Self::spans`]; empty out of bounds.
    pub fn span_range(&self, gx: i32, gz: i32) -> std::ops::Range<usize> {
        if !self.in_bounds(gx, gz) {
            return 0..0;
        }
        let i = self.index(gx, gz);
        self.cell_start[i] as usize..self.cell_start[i + 1] as usize
    }

    /// A cell's walkable spans, bottom-up; empty out of bounds.
    pub fn spans_at(&self, gx: i32, gz: i32) -> &[NavSpan] {
        &self.spans[self.span_range(gx, gz)]
    }

    /// Whether a cell holds any walkable span.
    pub fn is_walkable(&self, gx: i32, gz: i32) -> bool {
        !self.span_range(gx, gz).is_empty()
    }

    /// Every walkable span with its cell, in storage order.
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

    /// Exact cells for several inputs pin `- min_x`, `/ spacing` and `round`.
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
        let g = NavigationGraph::new(0.0, 10.0, 0.0, 10.0, 1.0); // width = 11
        assert_eq!(g.index(0, 0), 0);
        assert_eq!(g.index(1, 0), 1);
        assert_eq!(g.index(0, 1), 11);
        assert_eq!(g.index(3, 2), 25);
    }

    #[test]
    fn span_refs_walks_every_span_with_its_cell() {
        let g = NavigationGraph::new(0.0, 2.0, 0.0, 1.0, 1.0); // 3 x 2
        let refs: Vec<_> = g.span_refs().collect();
        assert_eq!(refs.len(), 6);
        assert_eq!((refs[4].gx, refs[4].gz, refs[4].index), (1, 1, 4));
    }
}
