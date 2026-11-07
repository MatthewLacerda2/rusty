//! The mesh a mesh-map bake reads (#1849), and what the maps derive from its
//! triangles: corners welded by position, the connected surface each triangle
//! belongs to, its face normal and its material.

use glam::{Vec2, Vec3};

use crate::asset::SubMesh;
use crate::core::collections::Map;
use crate::scene::lighting::lightmap::{world_per_uv, Surface};

/// The mesh a bake reads: unit normals, UV0, the triangles, and the material
/// runs of `indices`: where each run ends, and the material it is drawn with.
#[derive(Clone, Debug, Default)]
pub struct MeshSource {
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub uvs: Vec<Vec2>,
    pub indices: Vec<u32>,
    pub runs: Vec<(u32, Option<usize>)>,
}

/// The mesh's corners welded by position: each vertex's corner id, and each
/// corner's position. `-0.0` and `0.0` are one corner.
pub(super) struct Weld {
    pub ids: Vec<u32>,
    pub corners: Vec<Vec3>,
}

impl MeshSource {
    /// An imported sub-mesh, in its own space.
    pub fn from_sub_mesh(mesh: &SubMesh) -> Self {
        let v = &mesh.vertices;
        Self {
            positions: v.iter().map(|v| Vec3::from(v.position)).collect(),
            normals: v
                .iter()
                .map(|v| Vec3::from(v.normal).try_normalize().unwrap_or(Vec3::Y))
                .collect(),
            uvs: v.iter().map(|v| Vec2::from(v.tex_coords)).collect(),
            indices: mesh.indices.clone(),
            runs: mesh
                .ranges
                .iter()
                .map(|r| (r.indices.end, r.material))
                .collect(),
        }
    }

    pub(super) fn surface(&self) -> Surface<'_> {
        Surface {
            positions: &self.positions,
            normals: &self.normals,
            uvs: &self.uvs,
            indices: &self.indices,
        }
    }

    /// The ID map's key for the triangle starting at index `first`: its material's
    /// index plus one, 0 for a triangle drawn with no material (or past every run).
    /// Two runs with one material share a key, as Substance colours by material.
    pub(super) fn material_key(&self, first: usize) -> usize {
        let run = self.runs.partition_point(|&(end, _)| end as usize <= first);
        self.runs
            .get(run)
            .and_then(|&(_, m)| m)
            .map_or(0, |m| m + 1)
    }

    /// The unit face normal of the triangle starting at index `first`, by its
    /// winding; `None` for a triangle without area.
    pub(super) fn face_normal(&self, first: usize) -> Option<Vec3> {
        let p = [0, 1, 2].map(|k| self.positions[self.indices[first + k] as usize]);
        (p[1] - p[0]).cross(p[2] - p[0]).try_normalize()
    }

    pub(super) fn weld(&self) -> Weld {
        let mut map: Map<[u32; 3], u32> = Map::default();
        let mut corners = Vec::new();
        let ids = self
            .positions
            .iter()
            .map(|&p| {
                let p = p + Vec3::ZERO;
                *map.entry(p.to_array().map(f32::to_bits))
                    .or_insert_with(|| {
                        corners.push(p);
                        corners.len() as u32 - 1
                    })
            })
            .collect();
        Weld { ids, corners }
    }

    /// Per triangle, the id of the connected surface it belongs to: triangles
    /// sharing a welded corner are one surface, so a crate standing on a separate
    /// floor is two.
    pub(super) fn components(&self, weld: &Weld) -> Vec<u32> {
        let mut parent: Vec<u32> = (0..weld.corners.len() as u32).collect();
        fn root(parent: &mut [u32], mut i: u32) -> u32 {
            while parent[i as usize] != i {
                let up = parent[parent[i as usize] as usize];
                parent[i as usize] = up;
                i = up;
            }
            i
        }
        let corner = |i: u32| weld.ids[i as usize];
        for t in self.indices.chunks_exact(3) {
            let a = root(&mut parent, corner(t[0]));
            for &k in &t[1..] {
                let b = root(&mut parent, corner(k));
                parent[b as usize] = a;
            }
        }
        self.indices
            .chunks_exact(3)
            .map(|t| root(&mut parent, corner(t[0])))
            .collect()
    }

    /// Why this mesh cannot be baked, if it can't: no triangles, an index past its
    /// vertices, a non-finite position or UV, or UVs that enclose no area.
    pub(super) fn check(&self) -> Result<(), String> {
        let n = self.positions.len();
        if self.indices.len() < 3 {
            return Err("has no triangles".into());
        }
        if self.normals.len() != n || self.uvs.len() != n {
            return Err("has a vertex without a normal or a UV".into());
        }
        if self.indices.iter().any(|&i| i as usize >= n) {
            return Err("has an index past its vertices".into());
        }
        if !self.positions.iter().all(|p| p.is_finite()) || !self.uvs.iter().all(|u| u.is_finite())
        {
            return Err("has a non-finite position or UV".into());
        }
        world_per_uv(&self.surface())
            .map(|_| ())
            .ok_or_else(|| "has no UV0 (its texture UVs enclose no area)".into())
    }
}
