//! src/asset/convert/fbx/ — FBX → glTF through `ufbx` (#1024).
//!
//! `ufbx` reads binary and ASCII FBX (6.x and 7.x) and does the format's hard half:
//! it converts the file's units to metres, bakes pivots and geometric transforms into the geometry, and fills a PBR view
//! of every material whatever its shading model. This module turns the file's axes
//! into glTF's (`basis`) and walks the result into a [`glb::Document`]: the node hierarchy, one glTF mesh per FBX mesh split
//! into a primitive per material, base-colour materials, and skins. Animation
//! stacks are not converted yet.

mod basis;
mod material;
mod mesh;

use super::glb;
use crate::asset::ImportError;
use crate::core::collections::Map;
use basis::Basis;
use std::path::Path;

/// Convert the FBX at `source` into a `.glb` at `out`.
pub fn convert(source: &Path, out: &Path) -> Result<(), ImportError> {
    // Units are ufbx's to convert; axes are `basis`'s (see there for why).
    let opts = ufbx::LoadOpts {
        target_unit_meters: 1.0,
        space_conversion: ufbx::SpaceConversion::ModifyGeometry,
        geometry_transform_handling: ufbx::GeometryTransformHandling::ModifyGeometry,
        generate_missing_normals: true,
        ignore_animation: true,
        ..Default::default()
    };
    let scene = ufbx::load_file(&source.to_string_lossy(), opts)
        .map_err(|e| ImportError::Parse(format!("{}: {e:?}", source.display())))?;
    let out_dir = out.parent().unwrap_or_else(|| Path::new(""));
    let document = document(&scene, out_dir);
    std::fs::write(out, glb::write(&document))
        .map_err(|e| ImportError::Parse(format!("writing {}: {e}", out.display())))
}

/// The glTF document for a loaded FBX scene. Texture URIs are made relative to
/// `out_dir`, where the `.glb` is written.
fn document(scene: &ufbx::Scene, out_dir: &Path) -> glb::Document {
    let mut doc = glb::Document {
        materials: scene
            .materials
            .iter()
            .map(|m| material::material(m, out_dir))
            .collect(),
        ..Default::default()
    };
    let materials: Map<u32, usize> = scene
        .materials
        .iter()
        .enumerate()
        .map(|(i, m)| (m.element.element_id, i))
        .collect();

    // Nodes first, in depth-first order, so every joint has an index before any
    // skin names it. ufbx's root node is synthetic: its children are the roots.
    let basis = Basis::new(&scene.settings.axes);
    let mut nodes: Map<u32, usize> = Map::default();
    for child in scene.root_node.children.iter() {
        let index = add_node(child, basis, &mut doc.nodes, &mut nodes);
        doc.roots.push(index);
    }

    let mut meshes: Map<u32, usize> = Map::default();
    for node in scene.nodes.iter() {
        let (Some(fbx_mesh), Some(&index)) = (&node.mesh, nodes.get(&node.element.element_id))
        else {
            continue;
        };
        let id = fbx_mesh.element.element_id;
        let mesh_index = *meshes.entry(id).or_insert_with(|| {
            let name = unique_name(&doc.meshes, &fbx_mesh.element.name, &node.element.name);
            doc.meshes
                .push(mesh::mesh(fbx_mesh, name, basis, &materials, &nodes));
            doc.meshes.len() - 1
        });
        doc.nodes[index].mesh = Some(mesh_index);
        if let Some(skin) = mesh::skin(fbx_mesh, basis, &nodes) {
            doc.skins.push(skin);
            doc.nodes[index].skin = Some(doc.skins.len() - 1);
        }
    }
    doc
}

/// Append `node` and its subtree, recording each FBX element id's glTF index.
fn add_node(
    node: &ufbx::Node,
    basis: Basis,
    out: &mut Vec<glb::Node>,
    ids: &mut Map<u32, usize>,
) -> usize {
    let index = out.len();
    let name = if node.element.name.is_empty() {
        format!("node_{index}")
    } else {
        node.element.name.to_string()
    };
    let local = basis.matrix(basis::trs(&node.local_transform));
    let (scale, rotation, translation) = local.to_scale_rotation_translation();
    let mut gltf_node = glb::Node::new(name);
    gltf_node.translation = translation.as_vec3().to_array();
    gltf_node.rotation = rotation.as_quat().normalize().to_array();
    gltf_node.scale = scale.as_vec3().to_array();
    out.push(gltf_node);
    ids.insert(node.element.element_id, index);
    let children: Vec<usize> = node
        .children
        .iter()
        .map(|child| add_node(child, basis, out, ids))
        .collect();
    out[index].children = children;
    index
}

/// A mesh name no earlier mesh has: the FBX mesh's own name, else its node's,
/// with `.1`, `.2`, … appended on a clash. Sub-object ids are scene references,
/// so two meshes must never share one.
fn unique_name(meshes: &[glb::Mesh], own: &str, node: &str) -> String {
    let base = [own, node, "mesh"]
        .into_iter()
        .find(|n| !n.is_empty())
        .unwrap_or("mesh");
    let taken = |name: &str| meshes.iter().any(|m| m.name == name);
    if !taken(base) {
        return base.to_string();
    }
    (1..)
        .map(|n| format!("{base}.{n}"))
        .find(|name| !taken(name))
        .unwrap_or_else(|| base.to_string())
}

#[cfg(test)]
mod tests;
