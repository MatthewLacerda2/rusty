//! `ufbx` converts units to metres and bakes pivots; axes are `basis`'s.
//!
//! A converter never logs or drops a `Result`: what it leaves out on purpose is a
//! [`Skip`], and any failure fails the conversion (a stamped cache must not cover it).
#![deny(
    clippy::let_underscore_must_use,
    clippy::unused_result_ok,
    clippy::print_stdout,
    clippy::print_stderr
)]

mod anim;
mod basis;
mod material;
mod mesh;
mod texture;

use super::stage;
use super::stage::hook::{self, Step};
use super::{glb, Converted, Skip, Staging};
use crate::ImportError;
use basis::Basis;
use rusty_core::collections::Map;
use std::path::Path;
use texture::Textures;

pub fn convert(source: &Path, to: &Staging) -> Result<Converted, ImportError> {
    let scene = load(source)?;
    let mut textures = Textures::new(to, scene.textures.iter().map(|t| &**t));
    let mut takes = Vec::new();
    let document = document(&scene, &mut textures, &mut takes)?;
    hook::io(Step::WriteGltf)
        .and_then(|()| stage::write(&to.glb, &glb::write(&document)))
        .map_err(|e| ImportError::Parse(format!("writing {}: {e}", to.glb.display())))?;
    let mut skipped = textures.skipped;
    skipped.append(&mut takes);
    Ok(Converted {
        skipped,
        depends_on: textures.looked_at,
    })
}

#[cfg(test)]
fn convert_to(source: &Path, out: &Path) -> Result<(), ImportError> {
    let converter = super::converter_for(source).expect("an FBX");
    super::refresh(converter, source, out)
}

#[cfg(test)]
fn fixture(name: &str) -> std::path::PathBuf {
    rusty_core::project::checkout()
        .join("tests/fixtures/models")
        .join(name)
}

/// Fresh: the shared cache would keep an older build's conversion.
#[cfg(test)]
fn convert_and_import(source: &Path, out: &Path) -> crate::ImportedAsset {
    convert_to(source, out).expect("the fixture converts");
    crate::gltf::import::import(out).expect("its glTF imports")
}

fn load(source: &Path) -> Result<ufbx::SceneRoot, ImportError> {
    let opts = ufbx::LoadOpts {
        target_unit_meters: 1.0,
        space_conversion: ufbx::SpaceConversion::ModifyGeometry,
        geometry_transform_handling: ufbx::GeometryTransformHandling::ModifyGeometry,
        generate_missing_normals: true,
        ..Default::default()
    };
    let failed = |e: String| ImportError::Parse(format!("{}: {e}", source.display()));
    hook::io(Step::ReadSource).map_err(|e| failed(e.to_string()))?;
    ufbx::load_file(&source.to_string_lossy(), opts).map_err(|e| failed(format!("{e:?}")))
}

fn document(
    scene: &ufbx::Scene,
    textures: &mut Textures,
    skipped: &mut Vec<Skip>,
) -> Result<glb::Document, ImportError> {
    let mut doc = glb::Document {
        materials: scene
            .materials
            .iter()
            .map(|m| material::material(m, textures))
            .collect::<Result<_, _>>()?,
        ..Default::default()
    };
    let materials: Map<u32, usize> = scene
        .materials
        .iter()
        .enumerate()
        .map(|(i, m)| (m.element.element_id, i))
        .collect();

    // Nodes first, so every joint has an index before a skin names it. ufbx's root is synthetic.
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
    doc.animations = anim::animations(scene, basis, &nodes, skipped);
    Ok(doc)
}

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
    // Split as the animation keys are, so a clip blends with the rest pose.
    let local = basis.local(&node.local_transform);
    let mut gltf_node = glb::Node::new(name);
    gltf_node.translation = local.translation;
    gltf_node.rotation = local.rotation;
    gltf_node.scale = local.scale;
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

/// Sub-object ids are scene references: two meshes must never share a name (`.1`, `.2` on a clash).
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
