//! src/asset/gltf_skin.rs — glTF `skin` (skeleton) extraction for #79.
//!
//! Turns a glTF `skin` into a pure-data [`SkinData`] (inverse-bind matrices +
//! bind-pose joint globals). Pure: `glam` + the `gltf` crate, never wgpu/egui.
//!
//! A skin is bound to a mesh by a *node* that references both a `mesh` and a
//! `skin`, so the skeleton is keyed by mesh index. Joint node transforms are local
//! in glTF; the bind-pose global of each joint is the product down the scene
//! hierarchy, then re-expressed relative to the skinned node (its inverse) so the
//! resulting palette is mesh-local — exactly what the GPU skinning path expects.

use super::mesh_data::SkinData;
use glam::Mat4;
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

/// Build the per-mesh skeletons. Each node binding both a `mesh` and a `skin`
/// yields one [`SkinData`] keyed by `mesh.index()`. Joints are emitted in
/// `skin.joints()` order — the order the per-vertex `JOINTS_0` indices reference.
pub fn skins_by_mesh(
    document: &gltf::Document,
    buffers: &[gltf::buffer::Data],
) -> HashMap<usize, SkinData> {
    let globals = node_globals(document);
    let mut out = HashMap::new();

    for node in document.nodes() {
        let (Some(mesh), Some(skin)) = (node.mesh(), node.skin()) else {
            continue;
        };
        let mesh_inv = globals
            .get(&node.index())
            .copied()
            .unwrap_or(Mat4::IDENTITY)
            .inverse();

        let reader = skin.reader(|b| buffers.get(b.index()).map(|d| d.0.as_slice()));
        let inverse_bind: Vec<Mat4> = match reader.read_inverse_bind_matrices() {
            Some(matrices) => matrices.map(|m| Mat4::from_cols_array_2d(&m)).collect(),
            // Omitted inverse-bind matrices are defined to be identity by the spec.
            None => skin.joints().map(|_| Mat4::IDENTITY).collect(),
        };
        let bind_global: Vec<Mat4> = skin
            .joints()
            .map(|joint| {
                mesh_inv
                    * globals
                        .get(&joint.index())
                        .copied()
                        .unwrap_or(Mat4::IDENTITY)
            })
            .collect();

        out.insert(
            mesh.index(),
            SkinData {
                inverse_bind,
                bind_global,
            },
        );
    }
    out
}
