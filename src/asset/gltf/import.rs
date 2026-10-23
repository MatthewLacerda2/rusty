//! src/asset/gltf/import.rs — glTF 2.0 import (`.gltf` + `.glb`), meshes,
//! materials and skin bindings.
//!
//! Reads static geometry, base-color material values, the skin (issue #79):
//! per-vertex `JOINTS_0`/`WEIGHTS_0` plus the skeleton (`skin`), and the
//! animation clips that drive that skeleton (issue #80, `anim`). Pure data —
//! the `gltf` crate + `glam`, never wgpu/egui/mlua.
//!
//! Each glTF *mesh* becomes one addressable `SubMesh`; its primitives are merged
//! (index offsets fixed up) into one vertex/index stream, since a primitive split
//! is a material boundary, not a separate addressable object. Topology is
//! normalized on the way in (#317): every triangle mode (`TRIANGLES`,
//! `TRIANGLE_STRIP`, `TRIANGLE_FAN`) becomes a plain triangle list, and
//! non-surface modes (`POINTS`/`LINES`/`LINE_*`) are skipped with a warning.

use super::{anim, material, skin};
use crate::asset::mesh_data::{ImportedAsset, MeshVertex, SkinData, SubMesh};
use crate::asset::ImportError;
use gltf::mesh::Mode;
use std::path::Path;

/// Import a `.gltf` or `.glb` file into addressable sub-meshes + materials.
/// Only the buffers are read: images are never decoded here (the renderer decodes
/// a texture by its path), so one the `gltf` crate can't decode, a TGA say, never
/// fails the model (#1082).
pub fn import(path: &Path) -> Result<ImportedAsset, ImportError> {
    import_from(path, None)
}

/// [`import`], for the glTF a conversion of `source` wrote: a texture the
/// conversion extracted and named comes back as a reference to it in `source`
/// (`convert::texture_ref`), never as its path in the cache (#1130).
pub fn import_from(path: &Path, source: Option<&str>) -> Result<ImportedAsset, ImportError> {
    let parse = |e: gltf::Error| ImportError::Parse(e.to_string());
    let gltf::Gltf { document, blob } = gltf::Gltf::open(path).map_err(parse)?;
    // External texture and buffer URIs resolve relative to the glTF file's own directory.
    let base_dir = path.parent().unwrap_or_else(|| Path::new(""));
    let buffers = gltf::import_buffers(&document, Some(base_dir), blob).map_err(parse)?;
    let materials = document
        .materials()
        .map(|m| material::material_data(m, base_dir, source))
        .collect();
    let skins = skin::skins_by_mesh(&document, &buffers);

    let sub_meshes = document
        .meshes()
        .map(|mesh| {
            let mut sub = sub_mesh_from_gltf(
                &mesh,
                &buffers,
                skins.get(&mesh.index()).cloned(),
                &document,
            );
            sub.node_name = node_name(&document, mesh.index());
            sub
        })
        .collect();

    Ok(ImportedAsset {
        sub_meshes,
        materials,
    })
}

/// The name of the first node (document order) that places mesh `mesh`, if named.
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
        Some(s) => anim::clips_for_skin(document, buffers, &s.joint_nodes),
        None => Vec::new(),
    };

    SubMesh {
        id,
        vertices,
        indices,
        material,
        skin,
        clips,
        node_name: None,
    }
}

/// Merge one glTF primitive's vertices + (offset-fixed) indices into the shared
/// streams. A primitive with no positions is skipped (the streams stay as-is); one
/// without normals is shaded flat, as the spec requires (#1217).
fn append_primitive(
    primitive: &gltf::Primitive,
    buffers: &[gltf::buffer::Data],
    vertices: &mut Vec<MeshVertex>,
    indices: &mut Vec<u32>,
) {
    let Some((own, local, has_normals)) = read_primitive(primitive, buffers) else {
        return;
    };
    let (own, local) = match has_normals {
        true => (own, local),
        false => crate::asset::flat_normals::flat_shaded(&own, &local),
    };
    let base = vertices.len() as u32;
    vertices.extend(own);
    indices.extend(local.into_iter().map(|i| i + base));
}

/// One primitive's vertices, its triangle list, and whether it brought normals
/// (when not, every normal is zero and no tangent is generated). `None` for a
/// primitive with no positions or a non-surface mode.
fn read_primitive(
    primitive: &gltf::Primitive,
    buffers: &[gltf::buffer::Data],
) -> Option<(Vec<MeshVertex>, Vec<u32>, bool)> {
    let reader = primitive.reader(|b| buffers.get(b.index()).map(|d| d.0.as_slice()));

    let positions: Vec<[f32; 3]> = reader.read_positions()?.collect();

    // A primitive without one normal per position gets flat normals (#1217).
    let normals: Option<Vec<[f32; 3]>> = reader
        .read_normals()
        .map(|n| n.collect::<Vec<_>>())
        .filter(|n| n.len() == positions.len());

    let uvs: Vec<[f32; 2]> = reader
        .read_tex_coords(0)
        .map(|t| t.into_f32().collect())
        .unwrap_or_else(|| vec![[0.0, 0.0]; positions.len()]);

    // The lightmap UV (#438): Blender exports its second UV map as `TEXCOORD_1`.
    let lightmap_uvs: Vec<[f32; 2]> = reader
        .read_tex_coords(1)
        .map(|t| t.into_f32().collect())
        .unwrap_or_default();

    // Skin bindings are per-primitive and index into `skin.joints()` order.
    let joints: Vec<[u16; 4]> = reader
        .read_joints(0)
        .map(|j| j.into_u16().collect())
        .unwrap_or_default();
    let weights: Vec<[f32; 4]> = reader
        .read_weights(0)
        .map(|w| w.into_f32().collect())
        .unwrap_or_default();

    // Local (pre-offset) indices, normalized to a triangle list (#317): needed both
    // for the merged stream and to generate tangents when the glTF declares none.
    let read: Option<Vec<u32>> = reader.read_indices().map(|r| r.into_u32().collect());
    // `None` for a non-surface mode; `triangle_list` logged the skip.
    let local = triangle_list(primitive.mode(), read, positions.len() as u32)?;
    // glTF `TANGENT` accessor when present, else generate from positions + UVs (#207).
    // Without normals the flat path makes its own.
    let tangents: Vec<[f32; 4]> = match &normals {
        Some(normals) => reader
            .read_tangents()
            .map(|t| t.collect())
            .unwrap_or_else(|| {
                crate::asset::tangents::generate_tangents(&positions, normals, &uvs, &local)
            }),
        None => Vec::new(),
    };

    let mut own: Vec<MeshVertex> = Vec::with_capacity(positions.len());
    for (i, position) in positions.iter().enumerate() {
        let normal = normals.as_ref().map_or([0.0; 3], |n| n[i]);
        let uv = uvs.get(i).copied().unwrap_or([0.0, 0.0]);
        let mut vertex = MeshVertex::new(*position, normal, uv);
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

/// Normalize a primitive's index stream to a triangle list per its topology
/// `mode` (#317). glTF's three triangle modes carry the same kind of surface
/// under different index conventions: `TRIANGLES` passes through untouched,
/// `TRIANGLE_STRIP` / `TRIANGLE_FAN` are rewritten. A non-indexed primitive
/// (`indices == None`) is the sequential run `0..vertex_count` under the same
/// rules — a bare `0..n` is only a valid triangle soup under `TRIANGLES`.
/// Non-surface modes (`POINTS`/`LINES`/`LINE_LOOP`/`LINE_STRIP`) return `None`
/// with a logged skip: feeding their indices to the triangle path would
/// silently corrupt geometry (and the #77 trimesh collider build).
fn triangle_list(mode: Mode, indices: Option<Vec<u32>>, vertex_count: u32) -> Option<Vec<u32>> {
    let indices = indices.unwrap_or_else(|| (0..vertex_count).collect());
    match mode {
        Mode::Triangles => Some(indices),
        Mode::TriangleStrip => Some(strip_triangles(&indices)),
        Mode::TriangleFan => Some(fan_triangles(&indices)),
        Mode::Points | Mode::Lines | Mode::LineLoop | Mode::LineStrip => {
            log::warn!("[Asset] skipping glTF primitive: mode {mode:?} is not a triangle surface");
            None
        }
    }
}

/// `TRIANGLE_STRIP` → triangle list: one triangle per index window `(i, i+1, i+2)`,
/// swapping the leading pair on odd triangles so every emitted triangle keeps the
/// strip's front-face winding.
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

/// `TRIANGLE_FAN` → triangle list: each consecutive rim edge pairs with the first
/// index (the fan's hub).
fn fan_triangles(fan: &[u32]) -> Vec<u32> {
    let Some((&hub, rim)) = fan.split_first() else {
        return Vec::new();
    };
    rim.windows(2).flat_map(|w| [hub, w[0], w[1]]).collect()
}
