//! One GameObject per glTF node on instantiation (Unity's model prefab).

use std::borrow::Cow;

use glam::{Mat4, Quat, Vec3};

use super::mesh_data::ImportedAsset;

#[derive(Clone, Debug, PartialEq)]
pub struct ModelNode {
    pub name: String,
    /// Index into [`ImportedAsset::nodes`], always earlier than this node; `None` for a root.
    pub parent: Option<usize>,
    /// Bone this node hangs from; its transform is relative to that bone.
    pub bone: Option<NodeBone>,
    pub translation: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
    pub mesh: Option<String>,
}

/// A bone a node hangs from: the joint's name and its rest pose relative to the skinned parent.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeBone {
    pub name: String,
    pub rest: Mat4,
}

impl ModelNode {
    pub fn root_of(mesh: &str) -> Self {
        ModelNode {
            name: mesh.to_string(),
            parent: None,
            bone: None,
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
            mesh: Some(mesh.to_string()),
        }
    }

    pub fn local(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.translation)
    }
}

impl ImportedAsset {
    /// One root per sub-mesh at the identity when the asset has no nodes.
    pub fn node_tree(&self) -> Cow<'_, [ModelNode]> {
        if self.nodes.is_empty() {
            let roots = self.sub_meshes.iter().map(|s| ModelNode::root_of(&s.id));
            Cow::Owned(roots.collect())
        } else {
            Cow::Borrowed(&self.nodes)
        }
    }

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

/// Model-space transforms, parents first; bone-held nodes sit at the bone's rest pose.
pub fn worlds(nodes: &[ModelNode]) -> Vec<Mat4> {
    let mut out: Vec<Mat4> = Vec::with_capacity(nodes.len());
    for node in nodes {
        let parent = node.parent.and_then(|p| out.get(p).copied());
        let bone = node.bone.as_ref().map_or(Mat4::IDENTITY, |b| b.rest);
        out.push(parent.unwrap_or(Mat4::IDENTITY) * bone * node.local());
    }
    out
}

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
