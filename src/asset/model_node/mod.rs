//! src/asset/model_node/ — a model's node hierarchy (#1083).
//!
//! A glTF file places its meshes with *nodes*: each node has a local transform,
//! a parent and, optionally, a mesh. Instantiating a whole model rebuilds that
//! tree as GameObjects, one per node (Unity's model prefab,
//! `scene::model_instance`), so a building kit keeps its layout and every pivot
//! survives. The tree is plain data on the [`ImportedAsset`], where an import
//! cache can hold it (#857).

use std::borrow::Cow;

use glam::{Mat4, Quat, Vec3};

use super::mesh_data::ImportedAsset;

/// One node of a model: its name, its parent (an index into the same list,
/// always earlier), its local transform and the sub-mesh it places.
#[derive(Clone, Debug, PartialEq)]
pub struct ModelNode {
    /// The node's name (the Blender object name); `node_<n>` for an unnamed one.
    pub name: String,
    /// The parent node's index in [`ImportedAsset::nodes`], `None` for a root.
    pub parent: Option<usize>,
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
    /// The id of the sub-mesh this node places, `None` for an empty node.
    pub mesh: Option<String>,
}

impl ModelNode {
    /// A root node at the identity transform placing sub-mesh `mesh`.
    pub fn root_of(mesh: &str) -> Self {
        ModelNode {
            name: mesh.to_string(),
            parent: None,
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
            mesh: Some(mesh.to_string()),
        }
    }

    /// The local transform as a matrix.
    pub fn local(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }
}

impl ImportedAsset {
    /// The model's node tree: [`nodes`](Self::nodes) as imported, or, for an
    /// asset that has none (an `.obj`, a hand-built one), one root per sub-mesh
    /// at the identity, so every sub-mesh is placed exactly once.
    pub fn node_tree(&self) -> Cow<'_, [ModelNode]> {
        if self.nodes.is_empty() {
            let roots = self.sub_meshes.iter().map(|s| ModelNode::root_of(&s.id));
            Cow::Owned(roots.collect())
        } else {
            Cow::Borrowed(&self.nodes)
        }
    }

    /// Each node's model-space transform, in [`node_tree`](Self::node_tree) order.
    pub fn node_worlds(&self) -> Vec<Mat4> {
        worlds(&self.node_tree())
    }

    /// The model's bounds with every mesh at its node's transform — what a whole
    /// model spawned at the origin covers — or `None` when no node places a mesh
    /// with geometry.
    pub fn model_bounds(&self) -> Option<(Vec3, Vec3)> {
        let nodes = self.node_tree();
        let worlds = worlds(&nodes);
        let placed = nodes.iter().zip(&worlds).filter_map(|(node, world)| {
            let (lo, hi) = self.sub_mesh(node.mesh.as_deref()?)?.bounds()?;
            Some(transformed_bounds(*world, lo, hi))
        });
        placed.reduce(|(a0, a1), (b0, b1)| (a0.min(b0), a1.max(b1)))
    }
}

/// The model-space transform of every node in `nodes` (parents come first).
pub fn worlds(nodes: &[ModelNode]) -> Vec<Mat4> {
    let mut out: Vec<Mat4> = Vec::with_capacity(nodes.len());
    for node in nodes {
        let parent = node.parent.and_then(|p| out.get(p).copied());
        out.push(parent.unwrap_or(Mat4::IDENTITY) * node.local());
    }
    out
}

/// The axis-aligned bounds of box `lo..hi` under `m`: its eight corners'.
fn transformed_bounds(m: Mat4, lo: Vec3, hi: Vec3) -> (Vec3, Vec3) {
    let corner = |i: u32| {
        let pick = |bit: u32, a: f32, b: f32| if i & bit == 0 { a } else { b };
        m.transform_point3(Vec3::new(
            pick(1, lo.x, hi.x),
            pick(2, lo.y, hi.y),
            pick(4, lo.z, hi.z),
        ))
    };
    (1..8)
        .map(corner)
        .fold((corner(0), corner(0)), |(a, b), p| (a.min(p), b.max(p)))
}

#[cfg(test)]
mod tests;
