//! src/asset/gltf_import.rs — glTF 2.0 import (`.gltf` + `.glb`), meshes + materials.
//!
//! Skinning and animation are out of scope here (issue #79); this reads static
//! geometry and the base-color material values. Pure data — the `gltf` crate +
//! `glam`, never wgpu/egui/mlua.
//!
//! Each glTF *mesh* becomes one addressable `SubMesh`; its primitives are merged
//! (index offsets fixed up) into one vertex/index stream, since a primitive split
//! is a material boundary, not a separate addressable object.

use super::mesh_data::{ImportedAsset, MaterialData, MeshVertex, SubMesh};
use super::ImportError;
use std::path::Path;

/// Import a `.gltf` or `.glb` file into addressable sub-meshes + materials.
/// `gltf::import` transparently handles embedded vs. external buffers and `.glb`.
pub fn import(path: &Path) -> Result<ImportedAsset, ImportError> {
    let (document, buffers, _images) =
        gltf::import(path).map_err(|e| ImportError::Parse(e.to_string()))?;

    let materials = document.materials().map(material_data).collect();

    let sub_meshes = document
        .meshes()
        .map(|mesh| sub_mesh_from_gltf(&mesh, &buffers))
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

fn sub_mesh_from_gltf(mesh: &gltf::Mesh, buffers: &[gltf::buffer::Data]) -> SubMesh {
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
        let reader = primitive.reader(|b| buffers.get(b.index()).map(|d| d.0.as_slice()));

        let Some(positions) = reader.read_positions() else {
            continue;
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

        let base = vertices.len() as u32;
        for (i, position) in positions.iter().enumerate() {
            let normal = normals.get(i).copied().unwrap_or([0.0, 1.0, 0.0]);
            let uv = uvs.get(i).copied().unwrap_or([0.0, 0.0]);
            vertices.push(MeshVertex::new(*position, normal, uv));
        }

        match reader.read_indices() {
            Some(read) => indices.extend(read.into_u32().map(|i| i + base)),
            // Non-indexed primitive: emit a trivial 0..n index run.
            None => indices.extend((0..positions.len() as u32).map(|i| i + base)),
        }
    }

    SubMesh {
        id,
        vertices,
        indices,
        material,
    }
}
