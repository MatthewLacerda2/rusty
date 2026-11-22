use glam::Vec3;
use serde::{Deserialize, Serialize};

use super::Vertex;
use rusty_asset::tangents::generate_tangents;

/// The `primitive_type` of a mesh from the scene's mesh library; `asset_ref` names the entry.
pub const SCENE_KIND: &str = "Scene";

/// Vertices one mesh takes: a runaway script loop fails with a message, not gigabytes of buffer.
pub const MAX_VERTICES: usize = 1 << 20;

/// The most indices one mesh takes: two triangles per vertex at the cap, about what
/// a closed grid of [`MAX_VERTICES`] vertices needs.
pub const MAX_INDICES: usize = 6 * MAX_VERTICES;

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

    /// Call it on data that passed `validate`: it indexes the positions unchecked.
    pub fn geometry(&self) -> (Vec<Vertex>, Vec<u32>) {
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
