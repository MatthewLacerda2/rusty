//! glTF 2.0 import (`.gltf` + `.glb`): meshes, materials and skin bindings. Each glTF
//! mesh is one `SubMesh`; its primitives merge into one stream, each primitive's indices
//! a `MaterialRange`. Every triangle mode becomes a triangle list; non-surface modes are
//! skipped with a warning.

use super::{anim, material, nodes, skin};
use crate::material_range::push_range;
use crate::mesh_data::{ImportedAsset, SkinData, SubMesh, Vertex};
use crate::tangents::Missing;
use crate::ImportError;
use gltf::mesh::Mode;
use std::path::Path;

/// Images are never decoded here, so an undecodable one (a TGA) never fails the model.
pub fn import(path: &Path) -> Result<ImportedAsset, ImportError> {
    import_from(path, None)
}

/// A texture the conversion extracted comes back as a reference to it in `source`,
/// never as its path in the cache.
pub fn import_from(path: &Path, source: Option<&str>) -> Result<ImportedAsset, ImportError> {
    import_as(path, source, Missing::Generate)
}

pub fn import_as(
    path: &Path,
    source: Option<&str>,
    missing: Missing,
) -> Result<ImportedAsset, ImportError> {
    let parse = |e: gltf::Error| ImportError::Parse(e.to_string());
    let gltf::Gltf { document, blob } = gltf::Gltf::open(path).map_err(parse)?;
    let base_dir = path.parent().unwrap_or_else(|| Path::new(""));
    let buffers = gltf::import_buffers(&document, Some(base_dir), blob).map_err(parse)?;
    let materials = document
        .materials()
        .map(|m| material::material_data(m, base_dir, source))
        .collect();
    let skins = skin::skins_by_mesh(&document, &buffers);

    let meshes: Vec<gltf::Mesh> = document.meshes().collect();
    let triangles = |m: &gltf::Mesh| m.primitives().map(|p| corners(&p) / 3).sum();
    let sub_meshes: Vec<SubMesh> = crate::cook::par::map(meshes, triangles, |mesh| {
        let mut sub = sub_mesh_from_gltf(
            &mesh,
            &buffers,
            skins.get(&mesh.index()).cloned(),
            &document,
            missing,
        );
        sub.node_name = node_name(&document, mesh.index());
        sub
    });

    let ids: Vec<&str> = sub_meshes.iter().map(|s| s.id.as_str()).collect();
    let nodes = nodes::model_nodes(&document, &ids);
    Ok(ImportedAsset {
        sub_meshes,
        materials,
        nodes,
    })
}

fn node_name(document: &gltf::Document, mesh: usize) -> Option<String> {
    document
        .nodes()
        .find(|n| n.mesh().is_some_and(|m| m.index() == mesh))
        .and_then(|n| n.name().map(str::to_string))
}

fn sub_mesh_from_gltf(
    mesh: &gltf::Mesh,
    buffers: &[gltf::buffer::Data],
    skin: Option<SkinData>,
    document: &gltf::Document,
    missing: Missing,
) -> SubMesh {
    let id = mesh
        .name()
        .map(str::to_string)
        .unwrap_or_else(|| format!("mesh_{}", mesh.index()));

    let mut vertices: Vec<Vertex> = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let mut ranges = Vec::new();

    for primitive in mesh.primitives() {
        let start = indices.len() as u32;
        append_primitive(&primitive, buffers, (&mut vertices, &mut indices), missing);
        let run = start..indices.len() as u32;
        push_range(&mut ranges, run, primitive.material().index());
    }

    let clips = match &skin {
        Some(s) => anim::clips_for_skin(document, buffers, &s.joint_nodes),
        None => Vec::new(),
    };

    SubMesh {
        id,
        vertices: vertices.into(),
        indices: indices.into(),
        ranges,
        skin,
        clips,
        node_name: None,
        clusters: None,
    }
}

fn append_primitive(
    primitive: &gltf::Primitive,
    buffers: &[gltf::buffer::Data],
    (vertices, indices): (&mut Vec<Vertex>, &mut Vec<u32>),
    missing: Missing,
) {
    let Some((own, local, has_normals)) = read_primitive(primitive, buffers, missing) else {
        return;
    };
    let (own, local) = match has_normals {
        true => (own, local),
        false => crate::flat_normals::flat_shaded(&own, &local, missing),
    };
    let base = vertices.len() as u32;
    vertices.extend(own);
    indices.extend(local.into_iter().map(|i| i + base));
}

fn read_primitive(
    primitive: &gltf::Primitive,
    buffers: &[gltf::buffer::Data],
    missing: Missing,
) -> Option<(Vec<Vertex>, Vec<u32>, bool)> {
    let reader = primitive.reader(|b| buffers.get(b.index()).map(|d| d.0.as_slice()));

    let positions: Vec<[f32; 3]> = reader.read_positions()?.collect();

    let normals: Option<Vec<[f32; 3]>> = reader
        .read_normals()
        .map(|n| n.collect::<Vec<_>>())
        .filter(|n| n.len() == positions.len());

    let uvs: Vec<[f32; 2]> = reader
        .read_tex_coords(0)
        .map(|t| t.into_f32().collect())
        .unwrap_or_else(|| vec![[0.0, 0.0]; positions.len()]);

    let lightmap_uvs: Vec<[f32; 2]> = reader
        .read_tex_coords(1)
        .map(|t| t.into_f32().collect())
        .unwrap_or_default();

    let joints: Vec<[u16; 4]> = reader
        .read_joints(0)
        .map(|j| j.into_u16().collect())
        .unwrap_or_default();
    let weights: Vec<[f32; 4]> = reader
        .read_weights(0)
        .map(|w| w.into_f32().collect())
        .unwrap_or_default();

    let read: Option<Vec<u32>> = reader.read_indices().map(|r| r.into_u32().collect());
    let local = triangle_list(primitive.mode(), read, positions.len() as u32)?;
    let tangents: Vec<[f32; 4]> = match &normals {
        Some(normals) => reader
            .read_tangents()
            .map(|t| t.collect())
            .unwrap_or_else(|| crate::tangents::fill(missing, &positions, normals, &uvs, &local)),
        None => Vec::new(),
    };

    let mut own: Vec<Vertex> = Vec::with_capacity(positions.len());
    for (i, position) in positions.iter().enumerate() {
        let normal = normals.as_ref().map_or([0.0; 3], |n| n[i]);
        let uv = uvs.get(i).copied().unwrap_or([0.0, 0.0]);
        let mut vertex = Vertex::new((*position).into(), normal.into(), uv);
        vertex.tangent = tangents.get(i).copied().unwrap_or([1.0, 0.0, 0.0, 1.0]);
        vertex.lightmap_uv = lightmap_uvs.get(i).copied().unwrap_or([0.0, 0.0]);
        if let (Some(j), Some(w)) = (joints.get(i), weights.get(i)) {
            let ji = [j[0] as u32, j[1] as u32, j[2] as u32, j[3] as u32];
            vertex = vertex.with_skin(ji, *w);
        }
        own.push(vertex);
    }
    Some((own, local, normals.is_some()))
}

fn corners(primitive: &gltf::Primitive) -> usize {
    let indices = primitive.indices();
    let positions = || primitive.get(&gltf::Semantic::Positions);
    indices.or_else(positions).map_or(0, |a| a.count())
}

/// Non-surface modes return `None`: their indices on the triangle path would corrupt
/// geometry.
fn triangle_list(mode: Mode, indices: Option<Vec<u32>>, vertex_count: u32) -> Option<Vec<u32>> {
    let indices = indices.unwrap_or_else(|| (0..vertex_count).collect());
    match mode {
        Mode::Triangles => Some(indices),
        Mode::TriangleStrip => Some(strip_triangles(&indices)),
        Mode::TriangleFan => Some(fan_triangles(&indices)),
        Mode::Points | Mode::Lines | Mode::LineLoop | Mode::LineStrip => {
            rusty_core::warn_held!(
                "[Asset] skipping glTF primitive: mode {mode:?} is not a triangle surface"
            );
            None
        }
    }
}

/// Swapping the leading pair on odd triangles keeps the strip's front-face winding.
fn strip_triangles(strip: &[u32]) -> Vec<u32> {
    strip
        .windows(3)
        .enumerate()
        .flat_map(|(i, w)| {
            if i % 2 == 0 {
                [w[0], w[1], w[2]]
            } else {
                [w[1], w[0], w[2]]
            }
        })
        .collect()
}

fn fan_triangles(fan: &[u32]) -> Vec<u32> {
    let Some((&hub, rim)) = fan.split_first() else {
        return Vec::new();
    };
    rim.windows(2).flat_map(|w| [hub, w[0], w[1]]).collect()
}
