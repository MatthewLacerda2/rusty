use glam::{Vec2, Vec3};

use crate::Scene;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FloorCell {
    pub floor: f32,
    /// World `y` of the next solid's underside, `f32::INFINITY` under the sky.
    pub ceiling: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FloorGrid {
    pub origin: Vec2,
    pub cell: f32,
    pub width: usize,
    pub depth: usize,
    pub cells: Vec<Option<FloorCell>>,
    /// Floors above a cell's lowest, `(cell, y)` in cell order, which `placement::liquid`
    /// binary-searches. Probes fill over them; rooms use the lowest floor alone.
    pub upper: Vec<(usize, f32)>,
    /// How far open cells stop short of walls; a room's box grows by it.
    pub inset: f32,
}

// Metres above the floor an actor's body starts and ends: lower is stepped over, higher
// is overhead.
const STEP: f32 = 0.4;
const HEAD: f32 = 1.8;
/// A kilometre square at half a metre; larger levels get a coarser cell.
pub const MAX_FLOOR_CELLS: usize = 4_000_000;

impl FloorGrid {
    pub fn centre(&self, x: usize, z: usize) -> Vec2 {
        self.origin + Vec2::new(x as f32, z as f32) * self.cell
    }

    pub fn from_static(scene: &Scene, cell: f32) -> Option<Self> {
        let boxes = static_boxes(scene);
        let own = |&&(id, _): &&(u32, (Vec3, Vec3))| scene.in_active_scene(id);
        let (min, max) = (boxes.iter().filter(own))
            .map(|(_, b)| *b)
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
            upper: Vec::new(),
            inset: cell * 0.5,
        };
        let boxes: Vec<(Vec3, Vec3)> = boxes.into_iter().map(|(_, b)| b).collect();
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

    /// Every cell the footprint overlaps at all, so a wall thinner than a cell still blocks one.
    fn for_each_under(&self, lo: Vec3, hi: Vec3, mut f: impl FnMut(usize)) {
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

fn cells_across(span: Vec2, cell: f32) -> (usize, usize) {
    let n = |extent: f32| (extent / cell).ceil().max(1.0) as usize;
    (n(span.x), n(span.y))
}

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

pub fn static_boxes(scene: &Scene) -> Vec<(u32, (Vec3, Vec3))> {
    let world = &scene.world;
    (world.ids_with_collider().into_iter())
        .filter(|&id| world.is_static(id) && world.is_active_in_hierarchy(id))
        .filter_map(|id| {
            let collider = world.collider(id).filter(|c| c.active && !c.is_trigger)?;
            Some((id, collider.calculate_world_aabb(world.world_pose(id)?)))
        })
        .collect()
}
