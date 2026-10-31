//! src/asset/obj_import.rs — Wavefront `.obj` import (static meshes only).
//!
//! `.obj` is the static-mesh path: no skinning, no animation. Each OBJ object/group
//! becomes one addressable `SubMesh`; the `.mtl` materials become the shared material
//! table. `tobj` splits an object at every `usemtl`, reporting the pieces as models
//! sharing a name: they are joined back into one sub-mesh, each piece a
//! `MaterialRange` with its own material (#1285). Pure data — `tobj` + `glam`, no GPU
//! types.

use super::material_range::push_range;
use super::mesh_data::{ImportedAsset, MaterialData, MeshVertex, SubMesh};
use super::ImportError;
use crate::asset::MapRef;
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
                .map(|t| MapRef::from(base_dir.join(t).to_string_lossy().into_owned()));
            MaterialData {
                name: m.name,
                base_color: [base[0], base[1], base[2], 1.0],
                base_color_map,
                ..Default::default()
            }
        })
        .collect();

    let mut sub_meshes: Vec<SubMesh> = Vec::new();
    for (i, model) in models.into_iter().enumerate() {
        let piece = sub_mesh_from_model(i, model);
        match sub_meshes.iter_mut().find(|s| s.id == piece.id) {
            Some(sub) => append_piece(sub, piece),
            None => sub_meshes.push(piece),
        }
    }

    Ok(ImportedAsset {
        sub_meshes,
        materials,
        nodes: Vec::new(),
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
            // Without `vn` lines the flat path below fills it in (#1217).
            let normal = if has_normals {
                [
                    mesh.normals[v * 3],
                    mesh.normals[v * 3 + 1],
                    mesh.normals[v * 3 + 2],
                ]
            } else {
                [0.0; 3]
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

    // `.obj` carries no tangents; generate them so normal maps work (#207).
    let (vertices, indices) = if has_normals {
        (
            with_generated_tangents(vertices, &mesh.indices),
            mesh.indices,
        )
    } else {
        super::flat_normals::flat_shaded(&vertices, &mesh.indices)
    };

    SubMesh {
        id,
        ranges: super::material_range::whole(indices.len(), mesh.material_id),
        vertices,
        indices,
        // `.obj` is static-mesh only: no skin/anim, ever.
        skin: None,
        clips: Vec::new(),
        node_name: None,
    }
}

/// Join `piece` (one `usemtl` run of the same object) onto `sub`: its vertices and
/// offset indices appended, its material run after `sub`'s.
fn append_piece(sub: &mut SubMesh, piece: SubMesh) {
    let base = sub.vertices.len() as u32;
    let start = sub.indices.len() as u32;
    sub.vertices.extend(piece.vertices);
    sub.indices.extend(piece.indices.iter().map(|i| i + base));
    for run in piece.ranges {
        let indices = run.indices.start + start..run.indices.end + start;
        push_range(&mut sub.ranges, indices, run.material);
    }
}

/// Fill each vertex's tangent from positions + UVs (`.obj` has no tangent data).
fn with_generated_tangents(mut vertices: Vec<MeshVertex>, indices: &[u32]) -> Vec<MeshVertex> {
    let positions: Vec<[f32; 3]> = vertices.iter().map(|v| v.position).collect();
    let normals: Vec<[f32; 3]> = vertices.iter().map(|v| v.normal).collect();
    let uvs: Vec<[f32; 2]> = vertices.iter().map(|v| v.tex_coords).collect();
    let tangents = super::tangents::generate_tangents(&positions, &normals, &uvs, indices);
    for (v, t) in vertices.iter_mut().zip(tangents) {
        v.tangent = t;
    }
    vertices
}
