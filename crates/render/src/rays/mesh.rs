use glam::Vec3;

use super::build::{build, Node};
use rusty_components::mesh::{clip_runs, Vertex};

/// A triangle's corners and its run (its material slot in the mesh).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Tri {
    pub a: [f32; 3],
    pub run: u32,
    pub b: [f32; 4],
    pub c: [f32; 4],
}

/// Per corner: UV0 in `xy`, the lightmap UV in `zw`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct TriUv {
    pub a: [f32; 4],
    pub b: [f32; 4],
    pub c: [f32; 4],
}

impl Tri {
    pub(crate) fn corners(&self) -> [Vec3; 3] {
        let at = |p: &[f32]| Vec3::new(p[0], p[1], p[2]);
        [at(&self.a), at(&self.b), at(&self.c)]
    }
}

/// One mesh's own tree, in its local space, its node and triangle indices from 0.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct MeshTree {
    pub nodes: Vec<Node>,
    pub tris: Vec<Tri>,
    pub uvs: Vec<TriUv>,
}

impl MeshTree {
    pub(crate) fn build(
        vertices: &[Vertex],
        indices: &[u32],
        ranges: impl Iterator<Item = std::ops::Range<u32>>,
    ) -> Self {
        let mut tris = Vec::new();
        let mut uvs = Vec::new();
        let runs = clip_runs(ranges, indices.len() as u32);
        for (run, range) in runs.iter().enumerate() {
            let range = range.start as usize..range.end as usize;
            for corners in indices[range].chunks_exact(3) {
                if let Some((tri, uv)) = triangle(vertices, corners, run as u32) {
                    tris.push(tri);
                    uvs.push(uv);
                }
            }
        }
        let bounds: Vec<(Vec3, Vec3)> = tris
            .iter()
            .map(|t| {
                let [a, b, c] = t.corners();
                (a.min(b).min(c), a.max(b).max(c))
            })
            .collect();
        let (nodes, order) = build(&bounds);
        Self {
            nodes,
            tris: order.iter().map(|&i| tris[i as usize]).collect(),
            uvs: order.iter().map(|&i| uvs[i as usize]).collect(),
        }
    }
}

fn triangle(vertices: &[Vertex], corners: &[u32], run: u32) -> Option<(Tri, TriUv)> {
    let v = |i: usize| vertices.get(corners[i] as usize);
    let (a, b, c) = (v(0)?, v(1)?, v(2)?);
    let p = |v: &Vertex| Vec3::from_array(v.position);
    let area = (p(b) - p(a)).cross(p(c) - p(a)).length_squared();
    if !(area > 0.0 && area.is_finite()) {
        return None;
    }
    let at = |v: &Vertex| [v.position[0], v.position[1], v.position[2], 0.0];
    let uv = |v: &Vertex| {
        [
            v.tex_coords[0],
            v.tex_coords[1],
            v.lightmap_uv[0],
            v.lightmap_uv[1],
        ]
    };
    let tri = Tri {
        a: a.position,
        run,
        b: at(b),
        c: at(c),
    };
    Some((
        tri,
        TriUv {
            a: uv(a),
            b: uv(b),
            c: uv(c),
        },
    ))
}
