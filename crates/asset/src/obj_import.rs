//! Static meshes only: no skinning, no animation. `tobj` splits an object at each `usemtl`; the
//! pieces are joined back into one sub-mesh, each a `MaterialRange`.

use super::material_range::push_range;
use super::mesh_data::{ImportedAsset, MaterialData, SubMesh, Vertex};
use super::tangents::Missing;
use super::ImportError;
use crate::MapRef;
use std::path::Path;

pub fn import(path: &Path) -> Result<ImportedAsset, ImportError> {
    import_as(path, Missing::Generate)
}

pub fn import_as(path: &Path, missing: Missing) -> Result<ImportedAsset, ImportError> {
    let opts = tobj::LoadOptions {
        triangulate: true,
        single_index: true,
        ..Default::default()
    };
    let (models, materials) =
        tobj::load_obj(path, &opts).map_err(|e| ImportError::Parse(e.to_string()))?;
    // A missing `.mtl` leaves the geometry usable: empty material table, not an error.
    let materials = materials.unwrap_or_default();

    // Only `Kd` and `map_Kd` are read; the rest takes `Default`.
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

    let numbered: Vec<(usize, tobj::Model)> = models.into_iter().enumerate().collect();
    let triangles = |(_, m): &(usize, tobj::Model)| m.mesh.indices.len() / 3;
    let pieces = crate::cook::par::map(numbered, triangles, |(i, model)| {
        sub_mesh_from_model(i, model, missing)
    });
    let mut sub_meshes: Vec<SubMesh> = Vec::new();
    for piece in pieces {
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

fn sub_mesh_from_model(index: usize, model: tobj::Model, missing: Missing) -> SubMesh {
    let mesh = model.mesh;
    let id = Some(model.name)
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| format!("mesh_{index}"));

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
            // Without `vn` lines the flat path below fills it in.
            let normal = if has_normals {
                [
                    mesh.normals[v * 3],
                    mesh.normals[v * 3 + 1],
                    mesh.normals[v * 3 + 2],
                ]
            } else {
                [0.0; 3]
            };
            // OBJ V runs bottom-up; flip to the engine's top-down UVs.
            let tex_coords = if has_uvs {
                [mesh.texcoords[v * 2], 1.0 - mesh.texcoords[v * 2 + 1]]
            } else {
                [0.0, 0.0]
            };
            Vertex::new(position.into(), normal.into(), tex_coords)
        })
        .collect();

    let (vertices, indices) = if has_normals {
        (
            with_tangents(vertices, &mesh.indices, missing),
            mesh.indices,
        )
    } else {
        super::flat_normals::flat_shaded(&vertices, &mesh.indices, missing)
    };

    SubMesh {
        id,
        ranges: super::material_range::whole(indices.len(), mesh.material_id),
        vertices: vertices.into(),
        indices: indices.into(),
        skin: None,
        clips: Vec::new(),
        node_name: None,
        clusters: None,
    }
}

fn append_piece(sub: &mut SubMesh, piece: SubMesh) {
    let base = sub.vertices.len() as u32;
    let start = sub.indices.len() as u32;
    sub.vertices.edit().extend_from_slice(&piece.vertices);
    sub.indices
        .edit()
        .extend(piece.indices.iter().map(|i| i + base));
    for run in piece.ranges {
        let indices = run.indices.start + start..run.indices.end + start;
        push_range(&mut sub.ranges, indices, run.material);
    }
}

fn with_tangents(mut vertices: Vec<Vertex>, indices: &[u32], missing: Missing) -> Vec<Vertex> {
    let positions: Vec<[f32; 3]> = vertices.iter().map(|v| v.position).collect();
    let normals: Vec<[f32; 3]> = vertices.iter().map(|v| v.normal).collect();
    let uvs: Vec<[f32; 2]> = vertices.iter().map(|v| v.tex_coords).collect();
    let tangents = super::tangents::fill(missing, &positions, &normals, &uvs, indices);
    for (v, t) in vertices.iter_mut().zip(tangents) {
        v.tangent = t;
    }
    vertices
}
