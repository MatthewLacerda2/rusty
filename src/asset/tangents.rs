//! Per-vertex tangent generation (MikkTSpace) for normal mapping.
//!
//! A normal map stores perturbations in *tangent space*, so the fragment shader
//! needs a per-vertex tangent basis (T, with the vertex normal N and the derived
//! bitangent B) to rotate them into world space. glTF supplies a `TANGENT` accessor
//! when the exporter wrote one; when it didn't — and for `.obj` and the engine's
//! procedural primitives — we synthesize tangents here so the same `t_normal`
//! sampling path works for every mesh.
//!
//! The basis is **MikkTSpace** (#756), through `bevy_mikktspace`: the space Blender
//! and the other bakers write normal maps in, and the one the glTF 2.0 spec tells a
//! loader to generate. Any other basis lights the same normal map with seams and
//! wrong shading wherever the two disagree. The crate is pure, dependency-free math
//! with a fixed internal sort seed, so the output is a function of the mesh alone.
//!
//! The output is the glTF-convention `[x, y, z, w]`: the unit tangent plus a
//! handedness sign `w` (±1) the shader uses to reconstruct the bitangent as
//! `w * cross(N, T)`.

use bevy_mikktspace::Geometry;
use glam::Vec3;

/// Generate one `[x, y, z, w]` MikkTSpace tangent per vertex. `indices` lists
/// triangles (triples). A vertex MikkTSpace can give no tangent (a degenerate face
/// with no usable neighbour, or one no triangle references) falls back to an
/// arbitrary unit tangent orthogonal to its normal, so the basis is always finite.
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
        log::warn!("MikkTSpace tangent generation failed ({e}); using fallback tangents");
    }
    mesh.tangents
        .into_iter()
        .zip(&normals)
        .map(|(t, n)| {
            t.filter(|t| is_unit_finite(t))
                .unwrap_or_else(|| fallback(*n))
        })
        .collect()
}

/// rusty's indexed triangle list, seen through MikkTSpace's face/corner interface.
/// Corners that share a vertex index share its position, normal and UV, so the
/// algorithm welds them and writes them the same tangent.
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

/// True when the tangent's xyz is finite and unit length, i.e. usable as a basis.
fn is_unit_finite(t: &[f32; 4]) -> bool {
    let v = Vec3::new(t[0], t[1], t[2]);
    v.is_finite() && (v.length() - 1.0).abs() < 1e-3
}

/// Any unit tangent orthogonal to `n`, right-handed — for vertices MikkTSpace skips.
fn fallback(n: Vec3) -> [f32; 4] {
    let axis = if n.x.abs() < 0.9 { Vec3::X } else { Vec3::Y };
    let t = n.cross(axis).try_normalize().unwrap_or(Vec3::X);
    [t.x, t.y, t.z, 1.0]
}

#[cfg(test)]
#[path = "tangents_tests.rs"]
mod tests;
