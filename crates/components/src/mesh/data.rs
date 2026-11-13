//! crates/components/src/mesh/data.rs — a mesh made from vertex data (#2059): Unity's
//! `Mesh` built from `vertices`, `triangles`, `normals` and `uv`.
//!
//! [`MeshData`] is what a script hands `Mesh.Create` and what the scene's mesh
//! library saves (`scene::mesh_library`): flat, 0-based, as authored. Normals left
//! out are computed when it is built (Unity's `RecalculateNormals`, area-weighted);
//! the bounds are always the vertices'. Building it is plain CPU work, so the sim
//! (a Mesh collider) reads the same geometry the renderer uploads.

use glam::Vec3;
use serde::{Deserialize, Serialize};

use super::Vertex;
use rusty_asset::tangents::generate_tangents;

/// The `primitive_type` of a mesh drawn from the scene's mesh library; its
/// `asset_ref` names the library entry.
pub const SCENE_KIND: &str = "Scene";

/// The most vertices one mesh takes: 2^20. A Dust2-sized level piece built in a
/// script stays far under it, and a runaway loop fails with a message instead of
/// allocating gigabytes of vertex buffer (Unity's 32-bit index format reaches 4
/// billion; a script-built table that size never fits in a Lua VM anyway).
pub const MAX_VERTICES: usize = 1 << 20;

/// The most indices one mesh takes: two triangles per vertex at the cap, about what
/// a closed grid of [`MAX_VERTICES`] vertices needs.
pub const MAX_INDICES: usize = 6 * MAX_VERTICES;

/// One mesh as data. `indices` lists triangles (triples, counter-clockwise from
/// the front, as every rusty mesh is). `normals` and `uvs`, when given, hold one
/// entry per position; empty means "compute" (normals) or "all zero" (UVs).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MeshData {
    pub positions: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub normals: Vec<[f32; 3]>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub uvs: Vec<[f32; 2]>,
}

impl MeshData {
    /// Why this data cannot be a mesh, or `Ok` when it can: a count past the caps,
    /// indices not in triangles or out of range, normals or UVs not one per vertex,
    /// a NaN or infinite value.
    pub fn validate(&self) -> Result<(), String> {
        let n = self.positions.len();
        if n > MAX_VERTICES {
            return Err(format!("{n} vertices, past the limit of {MAX_VERTICES}"));
        }
        let i = self.indices.len();
        if i > MAX_INDICES {
            return Err(format!("{i} indices, past the limit of {MAX_INDICES}"));
        }
        if !i.is_multiple_of(3) {
            return Err(format!("{i} indices is not a whole number of triangles"));
        }
        if let Some((at, index)) = self
            .indices
            .iter()
            .enumerate()
            .find(|(_, &x)| x as usize >= n)
        {
            return Err(format!("index {at} is {index}, but there are {n} vertices"));
        }
        for (what, len) in [("normals", self.normals.len()), ("uvs", self.uvs.len())] {
            if len != 0 && len != n {
                return Err(format!("{len} {what} for {n} vertices"));
            }
        }
        let finite = |what: &str, at: Option<usize>| match at {
            Some(at) => Err(format!("{what} {at} is not a finite number")),
            None => Ok(()),
        };
        finite(
            "position",
            self.positions.iter().position(|p| !all_finite(p)),
        )?;
        finite("normal", self.normals.iter().position(|p| !all_finite(p)))?;
        finite("uv", self.uvs.iter().position(|p| !all_finite(p)))
    }

    /// The vertices and indices a `MeshComponent` draws: the given normals, or
    /// area-weighted ones; MikkTSpace tangents from the UVs, as every mesh gets; no
    /// lightmap UV (a static one reads light probes, as a mesh without one does).
    /// Call it on data that passed [`Self::validate`].
    pub fn geometry(&self) -> (Vec<Vertex>, Vec<u32>) {
        // A zero normal is given its +Y fallback before the tangents read it.
        let normals: Vec<[f32; 3]> = match self.normals.is_empty() {
            true => area_weighted_normals(&self.positions, &self.indices),
            false => (self.normals.iter())
                .map(|n| unit_or_up(Vec3::from_array(*n)))
                .collect(),
        };
        let uvs = match self.uvs.is_empty() {
            true => vec![[0.0; 2]; self.positions.len()],
            false => self.uvs.clone(),
        };
        let tangents = generate_tangents(&self.positions, &normals, &uvs, &self.indices);
        let vertices = (self.positions.iter().zip(&normals).zip(&uvs).zip(tangents))
            .map(|(((p, n), uv), t)| {
                let mut v = Vertex::new(Vec3::from_array(*p), Vec3::from_array(*n), *uv);
                v.tangent = t;
                v
            })
            .collect();
        (vertices, self.indices.clone())
    }
}

fn all_finite<const N: usize>(v: &[f32; N]) -> bool {
    v.iter().all(|x| x.is_finite())
}

/// Each vertex's normal as the sum of its triangles' face normals, each weighted by
/// its area (the unnormalised cross product), then normalised: Unity's
/// `RecalculateNormals`. A vertex no triangle with area touches points up.
pub fn area_weighted_normals(positions: &[[f32; 3]], indices: &[u32]) -> Vec<[f32; 3]> {
    let mut sums = vec![Vec3::ZERO; positions.len()];
    for tri in indices.chunks_exact(3) {
        let [a, b, c] = [tri[0], tri[1], tri[2]].map(|i| Vec3::from_array(positions[i as usize]));
        let face = (b - a).cross(c - a);
        for &i in tri {
            sums[i as usize] += face;
        }
    }
    sums.into_iter().map(unit_or_up).collect()
}

/// `n` normalised, or +Y when it has no length.
fn unit_or_up(n: Vec3) -> [f32; 3] {
    n.try_normalize().unwrap_or(Vec3::Y).to_array()
}

#[cfg(test)]
#[path = "data_tests.rs"]
mod tests;
