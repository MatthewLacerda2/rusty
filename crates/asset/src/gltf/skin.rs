//! A glTF `skin` as pure-data [`SkinData`]: inverse-bind matrices, bind-pose joint
//! globals, each joint's local bind TRS and parent slot, its source node index and the
//! skinned node's inverse. A skin binds to a mesh through a node referencing both, so
//! the skeleton is keyed by mesh index. Globals are re-expressed relative to the skinned
//! node so the palette is mesh-local, as the GPU skinning path expects.

use super::import::File;
use crate::mesh_data::{JointTransform, SkinData, Vertex};
use glam::{Mat4, Quat, Vec3};
use rusty_core::collections::Map;

fn node_globals(document: &gltf::Document) -> Map<usize, Mat4> {
    let mut globals = Map::default();
    for scene in document.scenes() {
        for node in scene.nodes() {
            walk(&node, Mat4::IDENTITY, &mut globals);
        }
    }
    globals
}

fn walk(node: &gltf::Node, parent: Mat4, out: &mut Map<usize, Mat4>) {
    let global = parent * Mat4::from_cols_array_2d(&node.transform().matrix());
    out.insert(node.index(), global);
    for child in node.children() {
        walk(&child, global, out);
    }
}

fn node_parents(document: &gltf::Document) -> Map<usize, usize> {
    let mut parents = Map::default();
    for node in document.nodes() {
        for child in node.children() {
            parents.insert(child.index(), node.index());
        }
    }
    parents
}

fn local_transform(node: &gltf::Node) -> JointTransform {
    let (t, r, s) = node.transform().decomposed();
    JointTransform {
        translation: Vec3::from_array(t),
        rotation: Quat::from_array(r),
        scale: Vec3::from_array(s),
    }
}

/// Joints are emitted in `skin.joints()` order, the order the per-vertex `JOINTS_0`
/// indices reference.
pub(super) fn skins_by_mesh(file: &File) -> Map<usize, SkinData> {
    let document = file.document;
    let globals = node_globals(document);
    let parents = node_parents(document);
    let mut out = Map::default();

    for node in document.nodes() {
        let (Some(mesh), Some(skin)) = (node.mesh(), node.skin()) else {
            continue;
        };
        let mesh_inverse = globals
            .get(&node.index())
            .copied()
            .unwrap_or(Mat4::IDENTITY)
            .inverse();
        out.insert(
            mesh.index(),
            build_skin(&skin, file, &globals, &parents, mesh_inverse),
        );
    }
    out
}

fn inverse_binds(skin: &gltf::Skin, file: &File, joints: usize) -> Vec<Mat4> {
    let reader = skin.reader(|b| file.data(b));
    let mut inverse_bind: Vec<Mat4> = match reader.read_inverse_bind_matrices() {
        Some(matrices) => matrices.map(|m| Mat4::from_cols_array_2d(&m)).collect(),
        None => Vec::new(),
    };
    if !inverse_bind.is_empty() && inverse_bind.len() != joints {
        log::warn!(
            "[Asset] {}: skin {}: {} inverse-bind matrices for {joints} joints; the missing are identity",
            file.name,
            skin.index(),
            inverse_bind.len(),
        );
    }
    // Omitted inverse-bind matrices are identity by the spec.
    inverse_bind.resize(joints, Mat4::IDENTITY);
    inverse_bind
}

fn build_skin(
    skin: &gltf::Skin,
    file: &File,
    globals: &Map<usize, Mat4>,
    parents: &Map<usize, usize>,
    mesh_inverse: Mat4,
) -> SkinData {
    let joint_nodes: Vec<usize> = skin.joints().map(|j| j.index()).collect();
    let slot_of: Map<usize, usize> = joint_nodes
        .iter()
        .enumerate()
        .map(|(slot, &node)| (node, slot))
        .collect();

    let inverse_bind = inverse_binds(skin, file, joint_nodes.len());
    let bind_global = skin
        .joints()
        .map(|joint| {
            mesh_inverse
                * globals
                    .get(&joint.index())
                    .copied()
                    .unwrap_or(Mat4::IDENTITY)
        })
        .collect();
    let local_bind = skin.joints().map(|j| local_transform(&j)).collect();
    let names = skin
        .joints()
        .enumerate()
        .map(|(slot, j)| {
            j.name()
                .map_or_else(|| format!("joint_{slot}"), str::to_string)
        })
        .collect();
    let joint_parents = joint_nodes
        .iter()
        .map(|node| parents.get(node).and_then(|p| slot_of.get(p)).copied())
        .collect();

    SkinData {
        inverse_bind,
        bind_global,
        local_bind,
        parents: joint_parents,
        joint_nodes,
        mesh_inverse,
        names,
    }
}

/// The palette has no matrix for a joint the skin lacks.
pub(super) fn drop_stray_influences(
    vertices: &mut [Vertex],
    joints: usize,
    file: &str,
    mesh: &str,
) {
    let mut dropped = 0;
    for v in vertices.iter_mut() {
        for (joint, weight) in v.joint_indices.iter_mut().zip(&mut v.joint_weights) {
            if *joint as usize >= joints {
                dropped += 1;
                (*joint, *weight) = (0, 0.0);
            }
        }
    }
    if dropped > 0 {
        rusty_core::warn_held!(
            "[Asset] {file}: mesh '{mesh}': {dropped} joint influences name none of the skin's {joints} joints; dropped"
        );
    }
}
