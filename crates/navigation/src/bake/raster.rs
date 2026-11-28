//! A convex collider gives one span per column, its lowest to highest triangle: a vertical line
//! crosses a convex solid in one interval, so its inside is solid. A non-convex mesh rasterises
//! per triangle and trusts the winding (front faces out), as Recast does.

use glam::{Vec2, Vec3};

use super::super::object_modifier::bake_as;
use super::super::{NavigationGraph, NOT_WALKABLE_AREA, WALKABLE_AREA};
use super::plane::{top_plane, Plane, TOP_EPS};
use super::region::CellRect;
use rusty_physics::collider_world_triangles;
use rusty_scene::Scene;

/// A fraction of the cell size, off each side: a collider only touching a cell's edge claims
/// none of it.
const EDGE_EPS: f32 = 1e-4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Solid {
    pub cell: u32,
    pub min: f32,
    pub max: f32,
    pub walkable: bool,
    pub plane: Option<Plane>,
    pub floor: f32,
    pub area: u8,
    pub overridden: bool,
}

/// A clipped polygon: at most 7 vertices (a triangle cut by four planes).
type Poly = ([Vec3; 8], usize);

pub(super) fn rasterize(
    g: &NavigationGraph,
    scene: &Scene,
    ids: &[u32],
    region: CellRect,
) -> (Vec<Solid>, Vec<Option<CellRect>>) {
    let mut out = Vec::new();
    let mut rects = Vec::with_capacity(ids.len());
    for &id in ids {
        let Some(mesh) = collider_world_triangles(scene, id) else {
            rects.push(None);
            continue;
        };
        rects.push(footprint(g, &mesh.vertices));
        let authored = bake_as(&scene.world, id).area;
        let area = authored.unwrap_or(WALKABLE_AREA);
        let surface = area != NOT_WALKABLE_AREA;
        let walkable = |tri| surface && faces_up(tri, g.max_slope, mesh.convex);
        let tris = mesh
            .triangles
            .iter()
            .map(|t| t.map(|i| mesh.vertices[i as usize]));
        let start = out.len();
        rasterize_mesh(g, region, tris, mesh.convex, walkable, &mut out);
        out[start..].iter_mut().for_each(|s| {
            s.area = area;
            s.overridden = authored.is_some();
        });
    }
    (out, rects)
}

pub(super) fn footprint(g: &NavigationGraph, vertices: &[Vec3]) -> Option<CellRect> {
    let lo = vertices.iter().copied().reduce(Vec3::min)?;
    let hi = vertices.iter().copied().reduce(Vec3::max)?;
    CellRect::covering(g, lo, hi)
}

pub(super) fn rasterize_mesh(
    g: &NavigationGraph,
    region: CellRect,
    triangles: impl Iterator<Item = [Vec3; 3]>,
    convex: bool,
    walkable: impl Fn([Vec3; 3]) -> bool,
    out: &mut Vec<Solid>,
) {
    let mut column = ColumnSink::new(convex);
    for tri in triangles {
        let walkable = walkable(tri);
        clip_into_cells(g, tri, region, |cell, centre, min, max| {
            column.add(Solid {
                cell,
                min,
                max,
                walkable,
                plane: walkable.then(|| Plane::of(tri, centre)).flatten(),
                floor: f32::NEG_INFINITY,
                area: WALKABLE_AREA,
                overridden: false,
            })
        });
    }
    column.drain_into(out);
}

struct ColumnSink {
    convex: bool,
    spans: Vec<Solid>,
}

impl ColumnSink {
    fn new(convex: bool) -> Self {
        Self {
            convex,
            spans: Vec::new(),
        }
    }

    fn add(&mut self, s: Solid) {
        self.spans.push(s);
    }

    fn drain_into(mut self, out: &mut Vec<Solid>) {
        if !self.convex {
            out.append(&mut self.spans);
            return;
        }
        self.spans.sort_by_key(|s| s.cell);
        for group in self.spans.chunk_by(|a, b| a.cell == b.cell) {
            let mut acc = group[0];
            for s in &group[1..] {
                acc.min = acc.min.min(s.min);
                acc.plane = top_plane((acc.max, acc.plane), (s.max, s.plane));
                if s.max > acc.max + TOP_EPS {
                    acc.walkable = s.walkable;
                } else if s.max >= acc.max - TOP_EPS {
                    acc.walkable |= s.walkable;
                }
                acc.max = acc.max.max(s.max);
            }
            out.push(acc);
        }
    }
}

fn faces_up(t: [Vec3; 3], max_slope: f32, convex: bool) -> bool {
    let n = (t[1] - t[0]).cross(t[2] - t[0]);
    // Convex: only the upper surface reaches a column's top, so either winding counts.
    let up = if convex { n.y.abs() } else { n.y };
    up > 0.0 && (n.x * n.x + n.z * n.z).sqrt() <= max_slope * up
}

fn clip_into_cells(
    g: &NavigationGraph,
    tri: [Vec3; 3],
    region: CellRect,
    mut emit: impl FnMut(u32, Vec2, f32, f32),
) {
    let Some(cells) = footprint(g, &tri).and_then(|f| f.intersection(region)) else {
        return;
    };
    let (x0, z0, x1, z1) = (cells.x0, cells.z0, cells.x1, cells.z1);
    let s = g.grid_spacing;
    let half = s * (0.5 - EDGE_EPS);
    let tri_poly: Poly = (
        [
            tri[0],
            tri[1],
            tri[2],
            Vec3::ZERO,
            Vec3::ZERO,
            Vec3::ZERO,
            Vec3::ZERO,
            Vec3::ZERO,
        ],
        3,
    );
    for gz in z0..=z1 {
        let cz = g.cell_center(0, gz).z;
        let row = clip(&clip(&tri_poly, 2, cz - half, true), 2, cz + half, false);
        if row.1 == 0 {
            continue;
        }
        for gx in x0..=x1 {
            let cx = g.cell_center(gx, 0).x;
            let cell = clip(&clip(&row, 0, cx - half, true), 0, cx + half, false);
            if cell.1 == 0 {
                continue;
            }
            let ys = cell.0[..cell.1].iter().map(|v| v.y);
            let (min, max) = ys.fold((f32::MAX, f32::MIN), |(a, b), y| (a.min(y), b.max(y)));
            emit(g.index(gx, gz) as u32, Vec2::new(cx, cz), min, max);
        }
    }
}

fn clip(poly: &Poly, axis: usize, bound: f32, keep_above: bool) -> Poly {
    let (src, n) = (&poly.0, poly.1);
    let mut out: Poly = ([Vec3::ZERO; 8], 0);
    let side = |v: Vec3| {
        let d = v[axis] - bound;
        if keep_above {
            d
        } else {
            -d
        }
    };
    for i in 0..n {
        let (a, b) = (src[i], src[(i + 1) % n]);
        let (da, db) = (side(a), side(b));
        if da >= 0.0 {
            push(&mut out, a);
        }
        if (da >= 0.0) != (db >= 0.0) {
            push(&mut out, a + (b - a) * (da / (da - db)));
        }
    }
    out
}

fn push(poly: &mut Poly, v: Vec3) {
    if poly.1 < poly.0.len() {
        poly.0[poly.1] = v;
        poly.1 += 1;
    }
}
