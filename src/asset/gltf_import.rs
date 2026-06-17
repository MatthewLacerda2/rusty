//! src/asset/gltf_import.rs — glTF 2.0 import (`.gltf` + `.glb`), meshes,
//! materials and skin bindings.
//!
//! Reads static geometry, base-color material values, the skin (issue #79):
//! per-vertex `JOINTS_0`/`WEIGHTS_0` plus the skeleton (`gltf_skin`), and the
//! animation clips that drive that skeleton (issue #80, `gltf_anim`). Pure data —
//! the `gltf` crate + `glam`, never wgpu/egui/mlua.
//!
//! Each glTF *mesh* becomes one addressable `SubMesh`; its primitives are merged
//! (index offsets fixed up) into one vertex/index stream, since a primitive split
//! is a material boundary, not a separate addressable object.

use super::mesh_data::{ImportedAsset, MaterialData, MeshVertex, SkinData, SubMesh};
use super::ImportError;
use super::{gltf_anim, gltf_skin};
use std::path::Path;

/// Import a `.gltf` or `.glb` file into addressable sub-meshes + materials.
/// `gltf::import` transparently handles embedded vs. external buffers and `.glb`.
pub fn import(path: &Path) -> Result<ImportedAsset, ImportError> {
    let (document, buffers, _images) =
        gltf::import(path).map_err(|e| ImportError::Parse(e.to_string()))?;

    let materials = document.materials().map(material_data).collect();
    let skins = gltf_skin::skins_by_mesh(&document, &buffers);

    let sub_meshes = document
        .meshes()
        .map(|mesh| {
            sub_mesh_from_gltf(
                &mesh,
                &buffers,
                skins.get(&mesh.index()).cloned(),
                &document,
            )
        })
        .collect();

    Ok(ImportedAsset {
        sub_meshes,
        materials,
    })
}

fn material_data(material: gltf::Material) -> MaterialData {
    let base_color = material.pbr_metallic_roughness().base_color_factor();
    MaterialData {
        name: material.name().unwrap_or("").to_string(),
        base_color,
    }
}

fn sub_mesh_from_gltf(
    mesh: &gltf::Mesh,
    buffers: &[gltf::buffer::Data],
    skin: Option<SkinData>,
    document: &gltf::Document,
) -> SubMesh {
    let id = mesh
        .name()
        .map(str::to_string)
        .unwrap_or_else(|| format!("mesh_{}", mesh.index()));

    let mut vertices: Vec<MeshVertex> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut material: Option<usize> = None;

    for primitive in mesh.primitives() {
        if material.is_none() {
            material = primitive.material().index();
        }
        append_primitive(&primitive, buffers, &mut vertices, &mut indices);
    }

    // Clips are keyed to this skin's joint slots, so only a skinned mesh carries
    // them (a static mesh has nothing for a channel to pose).
    let clips = match &skin {
        Some(s) => gltf_anim::clips_for_skin(document, buffers, &s.joint_nodes),
        None => Vec::new(),
    };

    SubMesh {
        id,
        vertices,
        indices,
        material,
        skin,
        clips,
    }
}

/// Merge one glTF primitive's vertices + (offset-fixed) indices into the shared
/// streams. A primitive with no positions is skipped (the streams stay as-is).
fn append_primitive(
    primitive: &gltf::Primitive,
    buffers: &[gltf::buffer::Data],
    vertices: &mut Vec<MeshVertex>,
    indices: &mut Vec<u32>,
) {
    let reader = primitive.reader(|b| buffers.get(b.index()).map(|d| d.0.as_slice()));

    let Some(positions) = reader.read_positions() else {
        return;
    };
    let positions: Vec<[f32; 3]> = positions.collect();

    let normals: Vec<[f32; 3]> = reader
        .read_normals()
        .map(|n| n.collect())
        .unwrap_or_else(|| vec![[0.0, 1.0, 0.0]; positions.len()]);

    let uvs: Vec<[f32; 2]> = reader
        .read_tex_coords(0)
        .map(|t| t.into_f32().collect())
        .unwrap_or_else(|| vec![[0.0, 0.0]; positions.len()]);

    // Skin bindings are per-primitive and index into `skin.joints()` order.
    let joints: Vec<[u16; 4]> = reader
        .read_joints(0)
        .map(|j| j.into_u16().collect())
        .unwrap_or_default();
    let weights: Vec<[f32; 4]> = reader
        .read_weights(0)
        .map(|w| w.into_f32().collect())
        .unwrap_or_default();

    let base = vertices.len() as u32;
    for (i, position) in positions.iter().enumerate() {
        let normal = normals.get(i).copied().unwrap_or([0.0, 1.0, 0.0]);
        let uv = uvs.get(i).copied().unwrap_or([0.0, 0.0]);
        let mut vertex = MeshVertex::new(*position, normal, uv);
        if let (Some(j), Some(w)) = (joints.get(i), weights.get(i)) {
            let ji = [j[0] as u32, j[1] as u32, j[2] as u32, j[3] as u32];
            vertex = vertex.with_skin(ji, *w);
        }
        vertices.push(vertex);
    }

    match reader.read_indices() {
        Some(read) => indices.extend(read.into_u32().map(|i| i + base)),
        // Non-indexed primitive: emit a trivial 0..n index run.
        None => indices.extend((0..positions.len() as u32).map(|i| i + base)),
    }
}
