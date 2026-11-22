//! A glTF `skin` as pure-data [`SkinData`]: inverse-bind matrices, bind-pose joint
//! globals, each joint's local bind TRS and parent slot, its source node index and the
//! skinned node's inverse. A skin binds to a mesh through a node referencing both, so
//! the skeleton is keyed by mesh index. Globals are re-expressed relative to the skinned
//! node so the palette is mesh-local, as the GPU skinning path expects.

use crate::mesh_data::{JointTransform, SkinData};
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
pub fn skins_by_mesh(
    document: &gltf::Document,
    buffers: &[gltf::buffer::Data],
) -> Map<usize, SkinData> {
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
            build_skin(&skin, buffers, &globals, &parents, mesh_inverse),
        );
    }
    out
}

fn build_skin(
    skin: &gltf::Skin,
    buffers: &[gltf::buffer::Data],
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

    let reader = skin.reader(|b| buffers.get(b.index()).map(|d| d.0.as_slice()));
    let inverse_bind = match reader.read_inverse_bind_matrices() {
        Some(matrices) => matrices.map(|m| Mat4::from_cols_array_2d(&m)).collect(),
        // Omitted inverse-bind matrices are identity by the spec.
        None => skin.joints().map(|_| Mat4::IDENTITY).collect(),
    };
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
