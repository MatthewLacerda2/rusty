//! One FBX mesh as a glTF mesh, and its skin.
//!
//! FBX stores attributes per polygon corner and polygons of any size; glTF wants
//! triangles over shared vertices. So each polygon is triangulated, each corner's
//! attributes read, and identical corners merged into one vertex. A primitive per
//! material part keeps material boundaries.

use super::basis::{self, Basis};
use super::glb;
use crate::core::collections::Map;

/// glTF carries four influences per vertex.
const MAX_INFLUENCES: usize = 4;

/// Corners with the same bits are one vertex: position, normal, UV, joints, weights.
type VertexKey = [u32; 16];

/// A corner's position, normal and UV.
type Corner = ([f32; 3], [f32; 3], [f32; 2]);

/// A vertex's four (joint slot, weight) influences.
type Influence = ([u16; 4], [f32; 4]);

/// `mesh` as a glTF mesh named `name`. `materials` maps an FBX material's element
/// id to its glTF index; `nodes` maps a node's element id to its glTF index.
pub fn mesh(
    mesh: &ufbx::Mesh,
    name: String,
    basis: Basis,
    materials: &Map<u32, usize>,
    nodes: &Map<u32, usize>,
) -> glb::Mesh {
    let influences = influences(mesh, nodes);
    let primitives = mesh
        .material_parts
        .iter()
        .filter(|part| part.num_triangles > 0)
        .map(|part| {
            let material = mesh
                .materials
                .get(part.index as usize)
                .and_then(|m| materials.get(&m.element.element_id).copied());
            primitive(mesh, part, basis, influences.as_deref(), material)
        })
        .collect();
    glb::Mesh { name, primitives }
}

/// The triangles of one material part.
fn primitive(
    mesh: &ufbx::Mesh,
    part: &ufbx::MeshPart,
    basis: Basis,
    influences: Option<&[Influence]>,
    material: Option<usize>,
) -> glb::Primitive {
    let mut out = glb::Primitive {
        material,
        ..Default::default()
    };
    let mut seen: Map<VertexKey, u32> = Map::default();
    let mut corners = Vec::with_capacity(mesh.max_face_triangles * 3);
    for &face in part.face_indices.iter() {
        let count = ufbx::triangulate_face_vec(&mut corners, mesh, mesh.faces[face as usize]);
        let triangles = &mut corners[..count as usize * 3];
        if basis.mirrored() {
            triangles.chunks_mut(3).for_each(|t| t.swap(1, 2));
        }
        for triangle in triangles.chunks(3) {
            let flat = flat_normal(mesh, basis, triangle);
            for &corner in triangle {
                let corner = corner as usize;
                let skin = influences.map(|v| v[mesh.vertex_indices[corner] as usize]);
                let (key, vertex) = corner_vertex(mesh, basis, corner, skin, flat);
                let index = *seen.entry(key).or_insert_with(|| {
                    push_vertex(&mut out, vertex, skin);
                    out.positions.len() as u32 - 1
                });
                out.indices.push(index);
            }
        }
    }
    out
}

/// The flat normal of one (already re-wound) triangle of corners, for a mesh with
/// no normals (#1217). The loader's `generate_missing_normals` means ufbx normally
/// fills them in first, so this is the backstop, not the usual path.
fn flat_normal(mesh: &ufbx::Mesh, basis: Basis, triangle: &[u32]) -> [f32; 3] {
    if mesh.vertex_normal.exists {
        return [0.0; 3]; // Unused: the corners carry their own.
    }
    let [a, b, c] = [0, 1, 2].map(|k| basis.vector(mesh.vertex_position[triangle[k] as usize]));
    crate::asset::flat_normals::face_normal(a, b, c)
}

/// What one corner contributes, and the key that merges it with its twins. `flat`
/// is its triangle's normal, used when the mesh has none of its own; corners keyed
/// on it merge only within one plane, so faces keep their own normal.
fn corner_vertex(
    mesh: &ufbx::Mesh,
    basis: Basis,
    corner: usize,
    skin: Option<Influence>,
    flat: [f32; 3],
) -> (VertexKey, Corner) {
    let position = basis.vector(mesh.vertex_position[corner]);
    let normal = if mesh.vertex_normal.exists {
        basis.vector(mesh.vertex_normal[corner])
    } else {
        flat
    };
    // FBX puts the UV origin bottom-left, glTF top-left.
    let uv = if mesh.vertex_uv.exists {
        let t = mesh.vertex_uv[corner];
        [t.x as f32, 1.0 - t.y as f32]
    } else {
        [0.0, 0.0]
    };
    let (joints, weights) = skin.unwrap_or_default();
    let mut key = [0u32; 16];
    let floats = position.iter().chain(&normal).chain(&uv).chain(&weights);
    for (slot, f) in key.iter_mut().zip(floats) {
        *slot = f.to_bits();
    }
    for (slot, j) in key[12..].iter_mut().zip(joints) {
        *slot = u32::from(j);
    }
    (key, (position, normal, uv))
}

fn push_vertex(out: &mut glb::Primitive, (position, normal, uv): Corner, skin: Option<Influence>) {
    out.positions.push(position);
    out.normals.push(normal);
    out.uvs.push(uv);
    if let Some((joints, weights)) = skin {
        out.joints.push(joints);
        out.weights.push(weights);
    }
}

/// The skin `mesh`'s first skin deformer binds it to: one joint per cluster whose
/// bone is a converted node, in cluster order.
pub fn skin(mesh: &ufbx::Mesh, basis: Basis, nodes: &Map<u32, usize>) -> Option<glb::Skin> {
    let deformer = mesh.skin_deformers.first()?;
    let mut skin = glb::Skin::default();
    for cluster in deformer.clusters.iter() {
        let Some(&joint) = bone_index(cluster, nodes) else {
            continue;
        };
        skin.joints.push(joint);
        let inverse_bind = basis.matrix(basis::to_dmat4(&cluster.geometry_to_bone));
        skin.inverse_bind
            .push(inverse_bind.as_mat4().to_cols_array());
    }
    (!skin.joints.is_empty()).then_some(skin)
}

fn bone_index<'a>(cluster: &ufbx::SkinCluster, nodes: &'a Map<u32, usize>) -> Option<&'a usize> {
    nodes.get(&cluster.bone_node.as_ref()?.element.element_id)
}

/// Per mesh vertex, its four strongest influences as (joint slot, weight), the
/// weights summing to one; `None` for an unskinned mesh. Joint slots count only
/// the clusters [`skin`] keeps, so the two agree.
fn influences(mesh: &ufbx::Mesh, nodes: &Map<u32, usize>) -> Option<Vec<Influence>> {
    let deformer = mesh.skin_deformers.first()?;
    let mut slots = Vec::with_capacity(deformer.clusters.len());
    let mut next = 0u16;
    for cluster in deformer.clusters.iter() {
        let kept = bone_index(cluster, nodes).is_some();
        slots.push(kept.then_some(next));
        next += u16::from(kept);
    }
    let per_vertex = deformer.vertices.iter().map(|v| {
        let range = v.weight_begin as usize..(v.weight_begin + v.num_weights) as usize;
        let mut weights: Vec<(u16, f32)> = deformer.weights.as_ref()[range]
            .iter()
            .filter_map(|w| {
                Some((
                    slots.get(w.cluster_index as usize).copied()??,
                    w.weight as f32,
                ))
            })
            .collect();
        weights.sort_by(|a, b| b.1.total_cmp(&a.1));
        strongest(&weights)
    });
    Some(per_vertex.collect())
}

/// The first [`MAX_INFLUENCES`] of `sorted` (strongest first), normalised; an
/// unweighted vertex follows joint 0 fully.
fn strongest(sorted: &[(u16, f32)]) -> Influence {
    let kept = &sorted[..sorted.len().min(MAX_INFLUENCES)];
    let total: f32 = kept.iter().map(|w| w.1).sum();
    if total <= 0.0 {
        return ([0; 4], [1.0, 0.0, 0.0, 0.0]);
    }
    let (mut joints, mut weights) = ([0u16; 4], [0f32; 4]);
    for (i, (joint, weight)) in kept.iter().enumerate() {
        joints[i] = *joint;
        weights[i] = weight / total;
    }
    (joints, weights)
}
