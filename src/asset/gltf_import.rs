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
//! is a material boundary, not a separate addressable object. Topology is
//! normalized on the way in (#317): every triangle mode (`TRIANGLES`,
//! `TRIANGLE_STRIP`, `TRIANGLE_FAN`) becomes a plain triangle list, and
//! non-surface modes (`POINTS`/`LINES`/`LINE_*`) are skipped with a warning.

use super::mesh_data::{ImportedAsset, MaterialData, MeshVertex, SkinData, SubMesh};
use super::ImportError;
use super::{gltf_anim, gltf_skin};
use gltf::mesh::Mode;
use std::path::Path;

/// Import a `.gltf` or `.glb` file into addressable sub-meshes + materials.
/// `gltf::import` transparently handles embedded vs. external buffers and `.glb`.
pub fn import(path: &Path) -> Result<ImportedAsset, ImportError> {
    let (document, buffers, _images) =
        gltf::import(path).map_err(|e| ImportError::Parse(e.to_string()))?;

    // External texture URIs resolve relative to the glTF file's own directory.
    let base_dir = path.parent().unwrap_or_else(|| Path::new(""));
    let materials = document
        .materials()
        .map(|m| material_data(m, base_dir))
        .collect();
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

/// Read one glTF material into the pure-data [`MaterialData`]: the metallic-roughness
/// factors plus the resolved file paths of its external textures (`base_dir` anchors
/// relative URIs). Embedded images yield `None` maps (see [`texture_path`]).
fn material_data(material: gltf::Material, base_dir: &Path) -> MaterialData {
    let pbr = material.pbr_metallic_roughness();
    MaterialData {
        name: material.name().unwrap_or("").to_string(),
        base_color: pbr.base_color_factor(),
        metallic: pbr.metallic_factor(),
        roughness: pbr.roughness_factor(),
        base_color_map: pbr
            .base_color_texture()
            .and_then(|t| texture_path(&t.texture(), base_dir)),
        metallic_roughness_map: pbr
            .metallic_roughness_texture()
            .and_then(|t| texture_path(&t.texture(), base_dir)),
        normal_map: material
            .normal_texture()
            .and_then(|t| texture_path(&t.texture(), base_dir)),
        emissive_map: material
            .emissive_texture()
            .and_then(|t| texture_path(&t.texture(), base_dir)),
        emissive: material.emissive_factor(),
    }
}

/// Resolve a glTF texture's image to a file path, anchored at the glTF file's
/// directory. Only the EXTERNAL-URI case is handled: an embedded image
/// (`Source::View`, a buffer view / data URI) returns `None` — extracting its pixels
/// to disk is a documented follow-up, out of scope for #203. The URI is
/// percent-decoded for the trivial `%20` (space) case; anything else is used as-is.
fn texture_path(texture: &gltf::Texture, base_dir: &Path) -> Option<String> {
    match texture.source().source() {
        gltf::image::Source::Uri { uri, .. } => {
            let decoded = uri.replace("%20", " ");
            Some(base_dir.join(decoded).to_string_lossy().into_owned())
        }
        // Embedded image: no on-disk path to reference (follow-up).
        gltf::image::Source::View { .. } => None,
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

    // Local (pre-offset) indices, normalized to a triangle list (#317): needed both
    // for the merged stream and to generate tangents when the glTF declares none.
    let read: Option<Vec<u32>> = reader.read_indices().map(|r| r.into_u32().collect());
    let Some(local) = triangle_list(primitive.mode(), read, positions.len() as u32) else {
        return; // Non-surface mode; `triangle_list` logged the skip.
    };
    // glTF `TANGENT` accessor when present, else generate from positions + UVs (#207).
    let tangents: Vec<[f32; 4]> =
        reader
            .read_tangents()
            .map(|t| t.collect())
            .unwrap_or_else(|| {
                crate::asset::tangents::generate_tangents(&positions, &normals, &uvs, &local)
            });

    let base = vertices.len() as u32;
    for (i, position) in positions.iter().enumerate() {
        let normal = normals.get(i).copied().unwrap_or([0.0, 1.0, 0.0]);
        let uv = uvs.get(i).copied().unwrap_or([0.0, 0.0]);
        let mut vertex = MeshVertex::new(*position, normal, uv);
        vertex.tangent = tangents.get(i).copied().unwrap_or([1.0, 0.0, 0.0, 1.0]);
        if let (Some(j), Some(w)) = (joints.get(i), weights.get(i)) {
            let ji = [j[0] as u32, j[1] as u32, j[2] as u32, j[3] as u32];
            vertex = vertex.with_skin(ji, *w);
        }
        vertices.push(vertex);
    }

    indices.extend(local.into_iter().map(|i| i + base));
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
