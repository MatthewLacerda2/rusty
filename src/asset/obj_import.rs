//! src/asset/obj_import.rs — Wavefront `.obj` import (static meshes only).
//!
//! `.obj` is the static-mesh path: no skinning, no animation. Each OBJ object/group
//! `tobj` reports becomes one addressable `SubMesh`; the `.mtl` materials become the
//! shared material table. Pure data — `tobj` + `glam`, no GPU types.

use super::mesh_data::{ImportedAsset, MaterialData, MeshVertex, SubMesh};
use super::ImportError;
use std::path::Path;

/// Import a `.obj` (resolving its sibling `.mtl` if present) into addressable
/// sub-meshes. Triangulated and single-indexed by `tobj`.
pub fn import(path: &Path) -> Result<ImportedAsset, ImportError> {
    let opts = tobj::LoadOptions {
        triangulate: true,
        single_index: true,
        ..Default::default()
    };
    let (models, materials) =
        tobj::load_obj(path, &opts).map_err(|e| ImportError::Parse(e.to_string()))?;
    // Materials may fail to load (missing `.mtl`) without the geometry being
    // unusable — treat that as an empty material table rather than a hard error.
    let materials = materials.unwrap_or_default();

    // OBJ is static-mesh only (per CLAUDE.md): we read just the trivially-available
    // values — `Kd` (base color) and `map_Kd` (base-color texture). All glTF-specific
    // fields (metallic/roughness, MR/normal/emissive maps) take their `Default`.
    let base_dir = path.parent().unwrap_or_else(|| Path::new(""));
    let materials: Vec<MaterialData> = materials
        .into_iter()
        .map(|m| {
            let base = m.diffuse.unwrap_or([1.0, 1.0, 1.0]);
            let base_color_map = m
                .diffuse_texture
                .map(|t| base_dir.join(t).to_string_lossy().into_owned());
            MaterialData {
                name: m.name,
                base_color: [base[0], base[1], base[2], 1.0],
                base_color_map,
                ..Default::default()
            }
        })
        .collect();

    let sub_meshes = models
        .into_iter()
        .enumerate()
        .map(|(i, model)| sub_mesh_from_model(i, model))
        .collect();

    Ok(ImportedAsset {
        sub_meshes,
        materials,
    })
}

fn sub_mesh_from_model(index: usize, model: tobj::Model) -> SubMesh {
    let mesh = model.mesh;
    let id = if model.name.is_empty() {
        format!("mesh_{index}")
    } else {
        model.name
    };

    let vertex_count = mesh.positions.len() / 3;
    let has_normals = mesh.normals.len() == mesh.positions.len();
    let has_uvs = mesh.texcoords.len() / 2 == vertex_count;

    let vertices = (0..vertex_count)
        .map(|v| {
            let position = [
                mesh.positions[v * 3],
                mesh.positions[v * 3 + 1],
                mesh.positions[v * 3 + 2],
            ];
            let normal = if has_normals {
                [
                    mesh.normals[v * 3],
                    mesh.normals[v * 3 + 1],
                    mesh.normals[v * 3 + 2],
                ]
            } else {
                [0.0, 1.0, 0.0]
            };
            // OBJ V runs bottom-up; flip to match the engine's top-down UVs.
            let tex_coords = if has_uvs {
                [mesh.texcoords[v * 2], 1.0 - mesh.texcoords[v * 2 + 1]]
            } else {
                [0.0, 0.0]
            };
            MeshVertex::new(position, normal, tex_coords)
        })
        .collect();

    SubMesh {
        id,
        vertices,
        indices: mesh.indices,
        material: mesh.material_id,
        // `.obj` is static-mesh only: no skin/anim, ever.
        skin: None,
        clips: Vec::new(),
    }
}
