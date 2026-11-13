//! Charting (#831): split a mesh's triangles into charts that each flatten onto one
//! plane without overlapping. A chart grows breadth-first from its seed (the lowest
//! unassigned triangle) across shared edges, taking a neighbour when
//!
//! * the edge between them is softer than the hard angle,
//! * the neighbour faces within [`CONE_DEGREES`] of the seed, so projecting it onto
//!   the seed's plane stretches it by at most `1 / cos` of that, and
//! * its projection overlaps none of the chart's triangles.
//!
//! A zero-area triangle joins any neighbour's chart: it has no surface to light.

use std::collections::VecDeque;

use glam::{Vec2, Vec3};

use super::overlap::OverlapGrid;

/// How far a chart's triangles may face from its seed, in degrees.
const CONE_DEGREES: f32 = 45.0;

/// Not yet in a chart.
const UNASSIGNED: u32 = u32::MAX;

/// One chart: its triangles (indices into the triangle list) and each one's corners
/// flattened onto the chart's plane, in world units.
#[derive(Clone, Debug, Default)]
pub(super) struct Chart {
    pub triangles: Vec<u32>,
    pub corners: Vec<[Vec2; 3]>,
}

/// The mesh as charting reads it.
struct Mesh<'a> {
    positions: &'a [[f32; 3]],
    indices: &'a [u32],
    normals: Vec<Vec3>,
    neighbours: Vec<Vec<u32>>,
    cos_hard: f32,
    cell: f32,
}

impl Mesh<'_> {
    fn corner(&self, t: u32, k: usize) -> Vec3 {
        Vec3::from(self.positions[self.indices[t as usize * 3 + k] as usize])
    }

    fn project(&self, t: u32, (u, v): (Vec3, Vec3)) -> [Vec2; 3] {
        [0, 1, 2].map(|k| {
            let p = self.corner(t, k);
            Vec2::new(p.dot(u), p.dot(v))
        })
    }

    /// Whether `next` may join the chart of `from`, whose seed faces `axis`.
    fn joins(&self, from: u32, next: u32, axis: Vec3) -> bool {
        let n = self.normals[next as usize];
        if n == Vec3::ZERO {
            return true;
        }
        let f = self.normals[from as usize];
        let f = if f == Vec3::ZERO { axis } else { f };
        n.dot(f) >= self.cos_hard && n.dot(axis) >= CONE_DEGREES.to_radians().cos()
    }
}

/// Split the triangle list `indices` into charts, every triangle in exactly one.
pub(super) fn build(positions: &[[f32; 3]], indices: &[u32], hard_angle: f32) -> Vec<Chart> {
    let triangles = indices.len() / 3;
    let mut mesh = Mesh {
        positions,
        indices,
        normals: Vec::with_capacity(triangles),
        neighbours: neighbours(positions, indices),
        cos_hard: hard_angle.to_radians().cos(),
        cell: 1.0,
    };
    mesh.normals = (0..triangles as u32)
        .map(|t| face_normal(&mesh, t))
        .collect();
    mesh.cell = mean_edge(&mesh);
    let mut chart_of = vec![UNASSIGNED; triangles];
    let mut charts = Vec::new();
    for seed in 0..triangles as u32 {
        if chart_of[seed as usize] == UNASSIGNED {
            let id = charts.len() as u32;
            charts.push(grow(&mesh, seed, id, &mut chart_of));
        }
    }
    charts
}

/// Which chart each triangle landed in.
pub(super) fn chart_of_triangle(charts: &[Chart]) -> Vec<u32> {
    let count = charts.iter().map(|c| c.triangles.len()).sum();
    let mut out = vec![0; count];
    for (id, chart) in charts.iter().enumerate() {
        for &t in &chart.triangles {
            out[t as usize] = id as u32;
        }
    }
    out
}

/// Grow chart `id` from `seed`, breadth-first in neighbour order.
fn grow(mesh: &Mesh, seed: u32, id: u32, chart_of: &mut [u32]) -> Chart {
    let normal = mesh.normals[seed as usize];
    let axis = if normal == Vec3::ZERO {
        Vec3::Y
    } else {
        normal
    };
    let basis = plane_basis(axis);
    let mut chart = Chart::default();
    let mut grid = OverlapGrid::new(mesh.cell);
    chart.take(&mut grid, seed, mesh.project(seed, basis));
    chart_of[seed as usize] = id;
    let mut queue = VecDeque::from([seed]);
    while let Some(from) = queue.pop_front() {
        for &next in &mesh.neighbours[from as usize] {
            if chart_of[next as usize] != UNASSIGNED || !mesh.joins(from, next, axis) {
                continue;
            }
            let corners = mesh.project(next, basis);
            if grid.overlaps(&corners) {
                continue;
            }
            chart.take(&mut grid, next, corners);
            chart_of[next as usize] = id;
            queue.push_back(next);
        }
    }
    chart
}

impl Chart {
    fn take(&mut self, grid: &mut OverlapGrid, t: u32, corners: [Vec2; 3]) {
        grid.insert(corners);
        self.triangles.push(t);
        self.corners.push(corners);
    }
}

/// Two unit axes spanning the plane facing `axis`. Walls get `v` straight up and
/// floors get `u` along `X`, so level geometry lands as axis-aligned rectangles that
/// pack tightly.
fn plane_basis(axis: Vec3) -> (Vec3, Vec3) {
    let u = if axis.y.abs() < 0.999 {
        Vec3::Y.cross(axis).normalize()
    } else {
        axis.cross(Vec3::Z).normalize()
    };
    (u, axis.cross(u))
}

/// Triangle `t`'s unit normal, or zero for a triangle with no area.
fn face_normal(mesh: &Mesh, t: u32) -> Vec3 {
    let [a, b, c] = [0, 1, 2].map(|k| mesh.corner(t, k));
    (b - a).cross(c - a).try_normalize().unwrap_or(Vec3::ZERO)
}

/// The mean edge length: the overlap grid's cell size.
fn mean_edge(mesh: &Mesh) -> f32 {
    let triangles = mesh.indices.len() / 3;
    let mut total = 0.0;
    for t in 0..triangles as u32 {
        let [a, b, c] = [0, 1, 2].map(|k| mesh.corner(t, k));
        total += a.distance(b) + b.distance(c) + c.distance(a);
    }
    let mean = total / (triangles * 3).max(1) as f32;
    if mean.is_finite() && mean > 0.0 {
        mean
    } else {
        1.0
    }
}

/// For each triangle, the triangles sharing an edge with it, ascending. Edges are
/// matched on welded positions, so a seam the exporter already split (a texture UV
/// or a hard normal) still connects.
fn neighbours(positions: &[[f32; 3]], indices: &[u32]) -> Vec<Vec<u32>> {
    let weld = weld(positions);
    let mut edges: Vec<(u32, u32, u32)> = Vec::with_capacity(indices.len());
    for (t, tri) in indices.chunks_exact(3).enumerate() {
        for k in 0..3 {
            let (a, b) = (weld[tri[k] as usize], weld[tri[(k + 1) % 3] as usize]);
            if a != b {
                edges.push((a.min(b), a.max(b), t as u32));
            }
        }
    }
    edges.sort_unstable();
    let mut out = vec![Vec::new(); indices.len() / 3];
    for run in edges.chunk_by(|x, y| (x.0, x.1) == (y.0, y.1)) {
        for (i, &(_, _, t)) in run.iter().enumerate() {
            for &(_, _, u) in &run[i + 1..] {
                out[t as usize].push(u);
                out[u as usize].push(t);
            }
        }
    }
    for list in &mut out {
        list.sort_unstable();
        list.dedup();
    }
    out
}

/// A shared id per distinct position (`-0.0` and `0.0` are one position).
fn weld(positions: &[[f32; 3]]) -> Vec<u32> {
    let key = |i: u32| positions[i as usize].map(|c| (c + 0.0).to_bits());
    let mut order: Vec<u32> = (0..positions.len() as u32).collect();
    order.sort_unstable_by_key(|&i| (key(i), i));
    let mut weld = vec![0; positions.len()];
    let mut id = 0;
    for (n, &i) in order.iter().enumerate() {
        if n > 0 && key(order[n - 1]) != key(i) {
            id += 1;
        }
        weld[i as usize] = id;
    }
    weld
}
