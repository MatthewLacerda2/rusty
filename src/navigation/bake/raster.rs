//! src/navigation/bake/raster.rs — static collider triangles → solid spans (#454).
//!
//! Recast's rasterisation step. Every static collider's world triangles
//! (`physics::collider_world_triangles`, the shape physics collides with) are
//! clipped against each cell column they cross: first to the cell row's z-strip,
//! then to the cell's x-range. The clipped polygon's lowest and highest `y` give a
//! solid span in that column, and the span's top is **walkable** when the triangle
//! that defines it faces up no steeper than `max_slope`.
//!
//! A convex collider (box, sphere, cylinder, capsule, hull) contributes **one** span
//! per column, from its lowest to its highest triangle: a vertical line crosses a
//! convex solid in a single interval, so the inside of a box is solid, not two thin
//! faces with phantom air between them. Its top is walkable when an up-facing
//! triangle reaches it; winding is not needed, since only the upper surface can
//! reach a convex solid's column maximum. A non-convex mesh rasterises triangle by
//! triangle and trusts the winding (front faces point out), as Recast does.
//!
//! A cell's column is its square around the centre, shrunk by [`EDGE_EPS`] so a
//! collider that only touches a cell's edge does not claim it.

use glam::Vec3;

use super::super::bounds::static_collider_ids;
use super::super::NavigationGraph;
use crate::physics::collider_world_triangles;
use crate::scene::Scene;

/// Fraction of the cell size the clip square is shrunk by on each side.
const EDGE_EPS: f32 = 1e-4;
/// Two triangle tops within this of each other count as the same top.
const TOP_EPS: f32 = 1e-3;

/// One solid interval in one cell column, as rasterised.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Solid {
    pub cell: u32,
    pub min: f32,
    pub max: f32,
    pub walkable: bool,
}

/// A clipped polygon: at most 7 vertices (a triangle cut by four planes).
type Poly = ([Vec3; 8], usize);

/// Rasterise every static collider of `scene` into solid spans on `g`'s grid.
pub(super) fn rasterize(g: &NavigationGraph, scene: &Scene) -> Vec<Solid> {
    let mut out = Vec::new();
    for id in static_collider_ids(scene) {
        let Some(mesh) = collider_world_triangles(scene, id) else {
            continue;
        };
        let mut column = ColumnSink::new(mesh.convex);
        for t in &mesh.triangles {
            let tri = t.map(|i| mesh.vertices[i as usize]);
            let walkable = faces_up(tri, g.max_slope, mesh.convex);
            clip_into_cells(g, tri, |cell, min, max| {
                column.add(Solid {
                    cell,
                    min,
                    max,
                    walkable,
                })
            });
        }
        column.drain_into(&mut out);
    }
    out
}

/// Collects one collider's spans: per triangle for a mesh, one per cell for a convex
/// solid (its lowest to highest point, walkable when an up-facing triangle is on top).
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
        // Group by cell (stable sort keeps triangle order), then fold each group.
        self.spans.sort_by_key(|s| s.cell);
        for group in self.spans.chunk_by(|a, b| a.cell == b.cell) {
            let mut acc = group[0];
            for s in &group[1..] {
                acc.min = acc.min.min(s.min);
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

/// Whether a triangle's top is a walkable surface: facing up (either way for a
/// convex solid, see the module docs) with a grade no steeper than `max_slope`.
fn faces_up(t: [Vec3; 3], max_slope: f32, convex: bool) -> bool {
    let n = (t[1] - t[0]).cross(t[2] - t[0]);
    let up = if convex { n.y.abs() } else { n.y };
    up > 0.0 && (n.x * n.x + n.z * n.z).sqrt() <= max_slope * up
}

/// Clip `tri` against every cell column it crosses, reporting `(cell, min_y, max_y)`.
fn clip_into_cells(g: &NavigationGraph, tri: [Vec3; 3], mut emit: impl FnMut(u32, f32, f32)) {
    let (lo, hi) = (
        tri[0].min(tri[1]).min(tri[2]),
        tri[0].max(tri[1]).max(tri[2]),
    );
    let s = g.grid_spacing;
    let cell_of = |v: f32, origin: f32| ((v - origin) / s).round() as i32;
    let (z0, z1) = (
        cell_of(lo.z, g.min_z).max(0),
        cell_of(hi.z, g.min_z).min(g.height - 1),
    );
    let (x0, x1) = (
        cell_of(lo.x, g.min_x).max(0),
        cell_of(hi.x, g.min_x).min(g.width - 1),
    );
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
        let cz = g.min_z + gz as f32 * s;
        let row = clip(&clip(&tri_poly, 2, cz - half, true), 2, cz + half, false);
        if row.1 == 0 {
            continue;
        }
        for gx in x0..=x1 {
            let cx = g.min_x + gx as f32 * s;
            let cell = clip(&clip(&row, 0, cx - half, true), 0, cx + half, false);
            if cell.1 == 0 {
                continue;
            }
            let ys = cell.0[..cell.1].iter().map(|v| v.y);
            let (min, max) = ys.fold((f32::MAX, f32::MIN), |(a, b), y| (a.min(y), b.max(y)));
            emit(g.index(gx, gz) as u32, min, max);
        }
    }
}

/// One Sutherland–Hodgman pass: keep the part of `poly` with coordinate `axis`
/// (0 = x, 2 = z) `>= bound` (`keep_above`) or `<= bound`.
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
