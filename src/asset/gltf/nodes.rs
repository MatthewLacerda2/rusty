//! src/asset/gltf/nodes.rs — a glTF file's node hierarchy as [`ModelNode`]s
//! (#1083).
//!
//! Walks the scene a viewer would show (the default `scene`, else the first, else
//! every parentless node) depth-first, so each node lands after its parent. A
//! skin's joints are left out: they spawn as the skinned mesh's bone children
//! (#453), and a node under a joint (a prop held in a hand) hangs from the nearest
//! node kept above it, its transform composed through the joints between. A node
//! reached twice (a malformed file whose hierarchy loops or shares a child) is
//! kept the first time only.

use glam::Mat4;

use crate::asset::model_node::ModelNode;

/// The nodes of `document`, each mesh named by `mesh_ids` (one per glTF mesh, in
/// document order).
pub fn model_nodes(document: &gltf::Document, mesh_ids: &[&str]) -> Vec<ModelNode> {
    let mut walk = Walk {
        mesh_ids,
        joint: vec![false; document.nodes().len()],
        seen: vec![false; document.nodes().len()],
        out: Vec::new(),
    };
    for joint in document
        .skins()
        .flat_map(|s| s.joints().collect::<Vec<_>>())
    {
        walk.joint[joint.index()] = true;
    }
    for root in roots(document) {
        walk.node(&root, None, Mat4::IDENTITY);
    }
    walk.out
}

/// The nodes the hierarchy starts from.
fn roots(document: &gltf::Document) -> Vec<gltf::Node<'_>> {
    if let Some(scene) = document
        .default_scene()
        .or_else(|| document.scenes().next())
    {
        return scene.nodes().collect();
    }
    let mut child = vec![false; document.nodes().len()];
    for node in document.nodes() {
        node.children().for_each(|c| child[c.index()] = true);
    }
    document.nodes().filter(|n| !child[n.index()]).collect()
}

struct Walk<'a> {
    mesh_ids: &'a [&'a str],
    joint: Vec<bool>,
    seen: Vec<bool>,
    out: Vec<ModelNode>,
}

impl Walk<'_> {
    /// Add `node` under the kept node `parent`, `carried` being the transform of
    /// the joints skipped between them, then its subtree.
    fn node(&mut self, node: &gltf::Node, parent: Option<usize>, carried: Mat4) {
        if std::mem::replace(&mut self.seen[node.index()], true) {
            return;
        }
        let local = carried * Mat4::from_cols_array_2d(&node.transform().matrix());
        if self.joint[node.index()] {
            for child in node.children() {
                self.node(&child, parent, local);
            }
            return;
        }
        let mesh = node.mesh().and_then(|m| self.mesh_ids.get(m.index()));
        let name = match (node.name(), mesh) {
            (Some(name), _) => name.to_string(),
            (None, Some(mesh)) => mesh.to_string(),
            (None, None) => format!("node_{}", node.index()),
        };
        let (scale, rotation, translation) = match carried == Mat4::IDENTITY {
            // Taken as authored, so a node's own TRS is never re-split.
            true => {
                let (t, r, s) = node.transform().decomposed();
                (s.into(), glam::Quat::from_array(r), t.into())
            }
            false => local.to_scale_rotation_translation(),
        };
        let index = self.out.len();
        self.out.push(ModelNode {
            name,
            parent,
            translation,
            rotation,
            scale,
            mesh: mesh.map(|m| m.to_string()),
        });
        for child in node.children() {
            self.node(&child, Some(index), Mat4::IDENTITY);
        }
    }
}

#[cfg(test)]
#[path = "nodes_tests.rs"]
mod tests;
