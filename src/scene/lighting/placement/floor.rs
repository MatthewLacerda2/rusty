//! The floor plan reflection probes are placed on (#1584): an XZ grid of cells, each
//! with the height of the floor an actor stands on there and the underside of what
//! is above it, or nothing where no one can stand (a wall, a pillar, the void).
//!
//! It comes from one of two places. With a baked nav surface the navigation side
//! builds it from its walkable spans (`NavigationGraph::floor_grid`) and hands it
//! over as data, keeping the arrow navigation → scene (#721); without one,
//! [`FloorGrid::from_static`] rasterizes the static colliders' bounds itself. The
//! room finder (`rooms.rs`) only ever sees this grid.
//!
//! Pure data + math — deterministic, no RNG/clock.

use glam::{Vec2, Vec3};

use crate::scene::Scene;

/// A floor-plan cell's open space, as the room finder reads it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FloorCell {
    /// World `y` an actor stands at.
    pub floor: f32,
    /// World `y` of the underside of the next solid above, `f32::INFINITY` under
    /// the sky.
    pub ceiling: f32,
}

/// An XZ grid of floor cells, row-major (`z * width + x`).
#[derive(Clone, Debug, PartialEq)]
pub struct FloorGrid {
    /// The world XZ of cell `(0, 0)`'s centre.
    pub origin: Vec2,
    /// A cell's edge, world units.
    pub cell: f32,
    pub width: usize,
    pub depth: usize,
    /// Each cell's open space; `None` where no one can stand.
    pub cells: Vec<Option<FloorCell>>,
    /// How far the open cells stop short of the walls: the nav surface's agent
    /// radius, half a cell for a grid rasterized from the colliders. A room's box is
    /// grown by it so it meets the walls.
    pub inset: f32,
}

/// Height above a floor an actor's body starts: anything lower is stepped over.
const STEP: f32 = 0.4;
/// Height above a floor an actor's body ends: anything higher is overhead.
const HEAD: f32 = 1.8;
/// The most cells a rasterized floor plan holds: 2000 × 2000, a kilometre square
/// at half a metre. A larger level is rasterized at a coarser cell.
pub const MAX_FLOOR_CELLS: usize = 4_000_000;

impl FloorGrid {
    /// The world XZ of cell `(x, z)`'s centre.
    pub fn centre(&self, x: usize, z: usize) -> Vec2 {
        self.origin + Vec2::new(x as f32, z as f32) * self.cell
    }

    /// The floor plan of `scene`'s static colliders at `cell` spacing, `None` with
    /// none to bound. From their world bounds: a cell's floor is the lowest top over
    /// it, it is blocked where a solid crosses body height above that floor, and its
    /// ceiling is the lowest bottom overhead. A collider is judged by its bounding
    /// box, so a level built as one mesh collider wants the nav surface's grid.
    /// A level too wide for [`MAX_FLOOR_CELLS`] at `cell` is rasterized at the
    /// finest cell that fits, with a warning naming its extent.
    pub fn from_static(scene: &Scene, cell: f32) -> Option<Self> {
        let boxes = static_boxes(scene);
        let (min, max) = boxes
            .iter()
            .copied()
            .reduce(|a, b| (a.0.min(b.0), a.1.max(b.1)))?;
        let span = Vec2::new(max.x - min.x, max.z - min.z);
        let cell = capped_cell(span, cell.max(0.1));
        let (width, depth) = cells_across(span, cell);
        let mut grid = Self {
            origin: Vec2::new(min.x, min.z) + Vec2::splat(cell * 0.5),
            cell,
            width,
            depth,
            cells: vec![None; width * depth],
            // Cells overlapping a wall are blocked, so the open ones stop about
            // half a cell short of its face.
            inset: cell * 0.5,
        };
        let mut floor = vec![f32::INFINITY; width * depth];
        for &(lo, hi) in &boxes {
            grid.for_each_under(lo, hi, |i| floor[i] = floor[i].min(hi.y));
        }
        let mut open: Vec<Option<FloorCell>> = (floor.iter())
            .map(|&f| {
                f.is_finite().then_some(FloorCell {
                    floor: f,
                    ceiling: f32::INFINITY,
                })
            })
            .collect();
        for &(lo, hi) in &boxes {
            grid.for_each_under(lo, hi, |i| {
                let Some(c) = &mut open[i] else { return };
                if lo.y >= c.floor + HEAD {
                    c.ceiling = c.ceiling.min(lo.y);
                } else if hi.y > c.floor + STEP {
                    open[i] = None;
                }
            });
        }
        grid.cells = open;
        Some(grid)
    }

    /// Call `f` with the index of every cell that overlaps `[lo, hi]`'s XZ
    /// footprint: conservatively, so a wall thinner than a cell still blocks one.
    fn for_each_under(&self, lo: Vec3, hi: Vec3, mut f: impl FnMut(usize)) {
        // Cell `i` spans `origin + (i ± 0.5) * cell`; it overlaps `[a, b]` when
        // `(a - origin) / cell - 0.5 < i < (b - origin) / cell + 0.5`.
        let range = |a: f32, b: f32, o: f32, n: usize| {
            let first = ((a - o) / self.cell - 0.5).floor() + 1.0;
            let last = ((b - o) / self.cell + 0.5).ceil() - 1.0;
            let last = last.min(n as f32 - 1.0);
            (last >= first.max(0.0)).then(|| first.max(0.0) as usize..=last as usize)
        };
        let Some(xs) = range(lo.x, hi.x, self.origin.x, self.width) else {
            return;
        };
        let Some(zs) = range(lo.z, hi.z, self.origin.y, self.depth) else {
            return;
        };
        for z in zs {
            for x in xs.clone() {
                f(z * self.width + x);
            }
        }
    }
}

/// How many cells of `cell` cover `span` along X and Z.
fn cells_across(span: Vec2, cell: f32) -> (usize, usize) {
    let n = |extent: f32| (extent / cell).ceil().max(1.0) as usize;
    (n(span.x), n(span.y))
}

/// `cell`, or the finest coarser cell at which `span` takes at most
/// [`MAX_FLOOR_CELLS`] (warned about, naming the extent).
fn capped_cell(span: Vec2, cell: f32) -> f32 {
    let fits = |cell: f32| {
        let (w, d) = cells_across(span, cell);
        w.saturating_mul(d) <= MAX_FLOOR_CELLS
    };
    if fits(cell) {
        return cell;
    }
    let mut coarse = cell.max((span.x * span.y / MAX_FLOOR_CELLS as f32).sqrt());
    while !fits(coarse) {
        coarse *= 1.01;
    }
    log::warn!(
        "[Lighting] the static level is {:.0} × {:.0} m, too wide for a {cell} m floor \
         plan; reflection probes are placed on a {coarse:.2} m one",
        span.x,
        span.y
    );
    coarse
}

/// The world bounds of the scene's static, active, solid colliders, in entity
/// order: a trigger is not a wall, as the nav bake has it.
fn static_boxes(scene: &Scene) -> Vec<(Vec3, Vec3)> {
    let world = &scene.world;
    (world.ids_with_collider().into_iter())
        .filter(|&id| world.is_static(id) && world.is_active_in_hierarchy(id))
        .filter_map(|id| {
            let collider = world.collider(id).filter(|c| c.active && !c.is_trigger)?;
            Some(collider.calculate_world_aabb(world.world_pose(id)?))
        })
        .collect()
}
