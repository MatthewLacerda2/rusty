//! Which sources can reach a point (#1883): a uniform grid over spheres of
//! influence, so a path vertex weighs only the lamps in range of it (and the
//! emitters near it), however many the level holds. A source lands in every cell
//! its sphere's box touches; one with no finite reach (a directional, a NaN or
//! unbounded range) is in every cell, through `global`. Built once per bake, read
//! by every vertex.

use glam::Vec3;

use super::super::input::{BakeLight, LightShape};

/// The most cells a grid spans along one axis.
const MAX_CELLS: f32 = 64.0;
/// The smallest cell edge, in world units.
const MIN_CELL: f32 = 2.0;

/// The sources by cell: `items[starts[c]..starts[c + 1]]` are cell `c`'s, each in
/// ascending lamp order, so a walk over them is deterministic.
#[derive(Debug, Default)]
pub(super) struct LightGrid {
    origin: Vec3,
    cell: f32,
    dims: [usize; 3],
    starts: Vec<u32>,
    items: Vec<u32>,
    global: Vec<u32>,
}

/// The sphere lamp `light` lights, if it has a finite one.
fn reach(light: &BakeLight) -> Option<(Vec3, f32)> {
    let (centre, range) = match light.shape {
        LightShape::Directional { .. } => return None,
        LightShape::Point { position, range }
        | LightShape::Spot {
            position, range, ..
        } => (position, range),
        LightShape::Area(area) => (area.center, area.range),
    };
    Some((centre, range))
}

impl LightGrid {
    /// The grid of `lights`, each by its sphere of influence.
    pub(super) fn build(lights: &[BakeLight]) -> Self {
        Self::of(lights.iter().map(reach).collect())
    }

    /// The grid of `spheres` (centre, radius), item `i` reaching only inside the
    /// `i`th; one with `None` (or not finite) reaches everywhere.
    pub(super) fn of(spheres: Vec<Option<(Vec3, f32)>>) -> Self {
        let finite =
            |s: Option<(Vec3, f32)>| s.filter(|(c, r)| c.is_finite() && r.is_finite() && *r >= 0.0);
        let spheres: Vec<_> = spheres.into_iter().map(finite).collect();
        let global = (0..spheres.len() as u32)
            .filter(|&i| spheres[i as usize].is_none())
            .collect();
        let bounded = spheres.iter().flatten();
        let lo = bounded
            .clone()
            .fold(Vec3::INFINITY, |m, (c, r)| m.min(*c - *r));
        let hi = bounded.fold(Vec3::NEG_INFINITY, |m, (c, r)| m.max(*c + *r));
        let mut grid = LightGrid {
            global,
            ..Default::default()
        };
        if !(lo.cmple(hi).all()) {
            return grid; // No bounded lamp.
        }
        let extent = (hi - lo).max_element();
        grid.cell = (extent / MAX_CELLS).max(MIN_CELL);
        grid.origin = lo;
        let span = |e: f32| ((e / grid.cell).floor() as usize + 1).min(MAX_CELLS as usize);
        let size = hi - lo;
        grid.dims = [span(size.x), span(size.y), span(size.z)];
        let mut buckets = vec![Vec::new(); grid.dims.iter().product()];
        for (i, sphere) in spheres.iter().enumerate() {
            let Some((c, r)) = sphere else { continue };
            let (a, b) = (grid.coords(*c - *r), grid.coords(*c + *r));
            for z in a[2]..=b[2] {
                for y in a[1]..=b[1] {
                    for x in a[0]..=b[0] {
                        buckets[grid.index([x, y, z])].push(i as u32);
                    }
                }
            }
        }
        grid.starts.push(0);
        for bucket in buckets {
            grid.items.extend(bucket);
            grid.starts.push(grid.items.len() as u32);
        }
        grid
    }

    /// The cell coordinates of `p`, clamped into the grid.
    fn coords(&self, p: Vec3) -> [usize; 3] {
        let f = (p - self.origin) / self.cell;
        let at = |v: f32, n: usize| (v.max(0.0) as usize).min(n - 1);
        [
            at(f.x, self.dims[0]),
            at(f.y, self.dims[1]),
            at(f.z, self.dims[2]),
        ]
    }

    fn index(&self, [x, y, z]: [usize; 3]) -> usize {
        (z * self.dims[1] + y) * self.dims[0] + x
    }

    /// Every lamp that may reach `p`: the unbounded ones, then those whose sphere
    /// touches `p`'s cell. A point outside every sphere's box gets only the first.
    pub(super) fn near(&self, p: Vec3) -> impl Iterator<Item = usize> + '_ {
        let inside = self.starts.len() > 1 && {
            let f = (p - self.origin) / self.cell;
            let dims = Vec3::new(
                self.dims[0] as f32,
                self.dims[1] as f32,
                self.dims[2] as f32,
            );
            f.cmpge(Vec3::ZERO).all() && f.cmple(dims).all()
        };
        let cell: &[u32] = match inside {
            true => {
                let c = self.index(self.coords(p));
                &self.items[self.starts[c] as usize..self.starts[c + 1] as usize]
            }
            false => &[],
        };
        self.global.iter().chain(cell).map(|&i| i as usize)
    }
}
