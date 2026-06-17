//! src/asset/gltf_skin.rs — glTF `skin` (skeleton) extraction for #79/#80.
//!
//! Turns a glTF `skin` into a pure-data [`SkinData`]: inverse-bind matrices, the
//! bind-pose joint globals (#79), and — for the animation runtime (#80) — each
//! joint's local bind TRS, its parent *slot* within the skin, its source node
//! index, and the skinned node's inverse. Pure: `glam` + the `gltf` crate, never
//! wgpu/egui.
//!
//! A skin is bound to a mesh by a *node* that references both a `mesh` and a
//! `skin`, so the skeleton is keyed by mesh index. Joint node transforms are local
//! in glTF; the bind-pose global of each joint is the product down the scene
//! hierarchy, then re-expressed relative to the skinned node (its inverse) so the
//! resulting palette is mesh-local — exactly what the GPU skinning path expects.

use super::mesh_data::{JointTransform, SkinData};
use glam::{Mat4, Quat, Vec3};
use std::collections::HashMap;

/// Global (model-space) transforms of every node, composed down the scene
/// hierarchy from each root. glTF stores only local transforms.
pub fn node_globals(document: &gltf::Document) -> HashMap<usize, Mat4> {
    let mut globals = HashMap::new();
    for scene in document.scenes() {
        for node in scene.nodes() {
            walk(&node, Mat4::IDENTITY, &mut globals);
        }
    }
    globals
}

fn walk(node: &gltf::Node, parent: Mat4, out: &mut HashMap<usize, Mat4>) {
    let global = parent * Mat4::from_cols_array_2d(&node.transform().matrix());
    out.insert(node.index(), global);
    for child in node.children() {
        walk(&child, global, out);
    }
}

/// Map each node to its parent node index (only nodes that have a parent appear).
/// glTF stores children, not parents, so we invert the hierarchy once.
fn node_parents(document: &gltf::Document) -> HashMap<usize, usize> {
    let mut parents = HashMap::new();
    for node in document.nodes() {
        for child in node.children() {
            parents.insert(child.index(), node.index());
        }
    }
    parents
}

/// Decompose a glTF node's local transform into a [`JointTransform`] (T/R/S) — the
/// rest pose the animation sampler overrides per channel.
fn local_transform(node: &gltf::Node) -> JointTransform {
    let (t, r, s) = node.transform().decomposed();
    JointTransform {
        translation: Vec3::from_array(t),
        rotation: Quat::from_array(r),
        scale: Vec3::from_array(s),
    }
}

/// Build the per-mesh skeletons. Each node binding both a `mesh` and a `skin`
/// yields one [`SkinData`] keyed by `mesh.index()`. Joints are emitted in
/// `skin.joints()` order — the order the per-vertex `JOINTS_0` indices reference.
pub fn skins_by_mesh(
    document: &gltf::Document,
    buffers: &[gltf::buffer::Data],
) -> HashMap<usize, SkinData> {
    let globals = node_globals(document);
    let parents = node_parents(document);
    let mut out = HashMap::new();

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

/// Assemble one [`SkinData`] from a glTF `skin`: the #79 bind-pose data plus the
/// #80 topology (local TRS, joint-slot parents, node indices, mesh inverse).
fn build_skin(
    skin: &gltf::Skin,
    buffers: &[gltf::buffer::Data],
    globals: &HashMap<usize, Mat4>,
    parents: &HashMap<usize, usize>,
    mesh_inverse: Mat4,
) -> SkinData {
    let joint_nodes: Vec<usize> = skin.joints().map(|j| j.index()).collect();
    // node index → joint slot, so parent lookups land on a slot within this skin.
    let slot_of: HashMap<usize, usize> = joint_nodes
        .iter()
        .enumerate()
        .map(|(slot, &node)| (node, slot))
        .collect();

    let reader = skin.reader(|b| buffers.get(b.index()).map(|d| d.0.as_slice()));
    let inverse_bind = match reader.read_inverse_bind_matrices() {
        Some(matrices) => matrices.map(|m| Mat4::from_cols_array_2d(&m)).collect(),
        // Omitted inverse-bind matrices are defined to be identity by the spec.
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
    }
}
