//! MikkTSpace tangents (`bevy_mikktspace`): the basis Blender and other bakers write normal maps
//! in, and the one glTF tells a loader to generate; any other basis shows seams. Output is
//! glTF-convention `[x, y, z, w]`, `w` (±1) the handedness so the bitangent is `w * cross(N, T)`.

use bevy_mikktspace::Geometry;
use glam::Vec3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Missing {
    Generate,
    /// The `+X` placeholder: a cooked import overwrites every tangent from its cooked file, so
    /// generating is wasted work.
    Placeholder,
}

pub fn fill(
    missing: Missing,
    positions: &[[f32; 3]],
    normals: &[[f32; 3]],
    uvs: &[[f32; 2]],
    indices: &[u32],
) -> Vec<[f32; 4]> {
    match missing {
        Missing::Generate => generate_tangents(positions, normals, uvs, indices),
        Missing::Placeholder => vec![[1.0, 0.0, 0.0, 1.0]; positions.len()],
    }
}

/// A vertex MikkTSpace can give no tangent (degenerate face, or unreferenced) falls back to an
/// arbitrary unit tangent orthogonal to its normal.
pub fn generate_tangents(
    positions: &[[f32; 3]],
    normals: &[[f32; 3]],
    uvs: &[[f32; 2]],
    indices: &[u32],
) -> Vec<[f32; 4]> {
    let normals: Vec<Vec3> = normals
        .iter()
        .map(|n| Vec3::from_array(*n).normalize_or_zero())
        .collect();
    let mut mesh = IndexedMesh {
        positions,
        normals: &normals,
        uvs,
        indices,
        tangents: vec![None; positions.len()],
    };
    if let Err(e) = bevy_mikktspace::generate_tangents(&mut mesh) {
        rusty_core::warn_held!(
            "[Asset] MikkTSpace tangent generation failed ({e}); using fallback tangents"
        );
    }
    mesh.tangents
        .into_iter()
        .zip(&normals)
        .map(|(t, n)| t.filter(is_unit_finite).unwrap_or_else(|| fallback(*n)))
        .collect()
}

struct IndexedMesh<'a> {
    positions: &'a [[f32; 3]],
    normals: &'a [Vec3],
    uvs: &'a [[f32; 2]],
    indices: &'a [u32],
    tangents: Vec<Option<[f32; 4]>>,
}

impl IndexedMesh<'_> {
    fn vertex(&self, face: usize, corner: usize) -> usize {
        self.indices[face * 3 + corner] as usize
    }
}

impl Geometry for IndexedMesh<'_> {
    fn num_faces(&self) -> usize {
        self.indices.len() / 3
    }

    fn num_vertices_of_face(&self, _face: usize) -> usize {
        3
    }

    fn position(&self, face: usize, vert: usize) -> [f32; 3] {
        self.positions[self.vertex(face, vert)]
    }

    fn normal(&self, face: usize, vert: usize) -> [f32; 3] {
        self.normals[self.vertex(face, vert)].to_array()
    }

    fn tex_coord(&self, face: usize, vert: usize) -> [f32; 2] {
        self.uvs[self.vertex(face, vert)]
    }

    fn set_tangent(
        &mut self,
        tangent_space: Option<bevy_mikktspace::TangentSpace>,
        face: usize,
        vert: usize,
    ) {
        let i = self.vertex(face, vert);
        self.tangents[i] = tangent_space.map(|ts| ts.tangent_encoded());
    }
}

fn is_unit_finite(t: &[f32; 4]) -> bool {
    let v = Vec3::new(t[0], t[1], t[2]);
    v.is_finite() && (v.length() - 1.0).abs() < 1e-3
}

fn fallback(n: Vec3) -> [f32; 4] {
    let axis = if n.x.abs() < 0.9 { Vec3::X } else { Vec3::Y };
    let t = n.cross(axis).try_normalize().unwrap_or(Vec3::X);
    [t.x, t.y, t.z, 1.0]
}

#[cfg(test)]
#[path = "tangents_tests.rs"]
mod tests;
