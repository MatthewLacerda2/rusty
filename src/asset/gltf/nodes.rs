//! src/asset/gltf/nodes.rs — a glTF file's node hierarchy as [`ModelNode`]s
//! (#1083).
//!
//! Walks the scene a viewer would show (the default `scene`, else the first, else
//! every parentless node) depth-first, so each node lands after its parent. A
//! joint of a skin some kept mesh node uses is left out: it spawns as that skinned
//! mesh's bone child (#453). What a joint holds hangs from the bone (Unity keeps a
//! gun under `Hand`): a non-joint child of it, and a mesh on the joint itself, are
//! kept with the skinned node as parent and the bone named ([`NodeBone`]), walked
//! after the main tree so the skinned node is always earlier. A joint no kept
//! mesh skins has no bone to spawn, so it is an ordinary node. A node reached
//! twice (a malformed file whose hierarchy loops or shares a child) is kept the
//! first time only.

use glam::{Mat4, Quat};

use crate::asset::model_node::{ModelNode, NodeBone};

/// The nodes of `document`, each mesh named by `mesh_ids` (one per glTF mesh, in
/// document order).
pub fn model_nodes<'a>(document: &'a gltf::Document, mesh_ids: &'a [&'a str]) -> Vec<ModelNode> {
    let roots = roots(document);
    let mut walk = Walk {
        mesh_ids,
        bone: bones(document, &roots),
        seen: vec![false; document.nodes().len()],
        kept: vec![None; document.nodes().len()],
        held: Vec::new(),
        out: Vec::new(),
    };
    for root in &roots {
        walk.node(root, None, None, Mat4::IDENTITY);
    }
    // What the bones hold, each under its skinned node, now walked.
    while !walk.held.is_empty() {
        for (held, owner, name, rest) in std::mem::take(&mut walk.held) {
            let parent = walk.kept[owner];
            let bone = Some(NodeBone { name, rest });
            match held {
                Held::Child(node) => walk.node(&node, parent, bone, Mat4::IDENTITY),
                Held::Mesh(node) => {
                    let at_bone = ([0.0; 3], Quat::IDENTITY.to_array(), [1.0; 3]);
                    walk.push(&node, parent, bone, at_bone);
                }
            }
        }
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

/// For each node that spawns as a bone: the skinned node (reachable from `roots`,
/// itself no joint) whose skin names it, and the bone's name in that skin.
fn bones(document: &gltf::Document, roots: &[gltf::Node]) -> Vec<Option<(usize, String)>> {
    let count = document.nodes().len();
    let mut reachable = vec![false; count];
    let mut stack: Vec<gltf::Node> = roots.to_vec();
    while let Some(node) = stack.pop() {
        if !std::mem::replace(&mut reachable[node.index()], true) {
            stack.extend(node.children());
        }
    }
    let mut joint = vec![false; count];
    for j in document
        .skins()
        .flat_map(|s| s.joints().collect::<Vec<_>>())
    {
        joint[j.index()] = true;
    }
    let mut out = vec![None; count];
    for node in document.nodes() {
        let skin = node.skin().filter(|_| node.mesh().is_some());
        let Some(skin) = skin.filter(|_| reachable[node.index()] && !joint[node.index()]) else {
            continue;
        };
        for (slot, j) in skin.joints().enumerate() {
            // Named as `asset::gltf::skin` names the bone it spawns.
            let name = j
                .name()
                .map_or_else(|| format!("joint_{slot}"), str::to_string);
            out[j.index()].get_or_insert((node.index(), name));
        }
    }
    out
}

/// A node a bone holds, waiting for its skinned node to be kept.
enum Held<'a> {
    /// A non-joint child of the joint, walked with its subtree.
    Child(gltf::Node<'a>),
    /// The mesh on the joint itself, placed at the bone.
    Mesh(gltf::Node<'a>),
}

struct Walk<'a> {
    mesh_ids: &'a [&'a str],
    bone: Vec<Option<(usize, String)>>,
    seen: Vec<bool>,
    /// Each glTF node's index in `out`, once kept.
    kept: Vec<Option<usize>>,
    /// What bones hold: the node, its skinned node, the bone and its rest.
    held: Vec<(Held<'a>, usize, String, Mat4)>,
    out: Vec<ModelNode>,
}

impl<'a> Walk<'a> {
    /// Add `node` under the kept node `parent` (from its `bone` when it hangs from
    /// one), then its subtree. `chain` is the rest pose of the bones above it when
    /// it is a bone itself, relative to the skinned node.
    fn node(
        &mut self,
        node: &gltf::Node<'a>,
        parent: Option<usize>,
        bone: Option<NodeBone>,
        chain: Mat4,
    ) {
        if std::mem::replace(&mut self.seen[node.index()], true) {
            return;
        }
        if let Some((owner, name)) = self.bone[node.index()].clone() {
            let rest = chain * Mat4::from_cols_array_2d(&node.transform().matrix());
            if node.mesh().is_some() {
                self.held
                    .push((Held::Mesh(node.clone()), owner, name.clone(), rest));
            }
            for child in node.children() {
                match self.bone[child.index()].is_some() {
                    true => self.node(&child, parent, None, rest),
                    false => self
                        .held
                        .push((Held::Child(child), owner, name.clone(), rest)),
                }
            }
            return;
        }
        let (t, r, s) = node.transform().decomposed();
        let index = self.push(node, parent, bone, (t, r, s));
        self.kept[node.index()] = Some(index);
        for child in node.children() {
            self.node(&child, Some(index), None, Mat4::IDENTITY);
        }
    }

    /// Keep `node` with its local translation, rotation and scale; its index.
    fn push(
        &mut self,
        node: &gltf::Node,
        parent: Option<usize>,
        bone: Option<NodeBone>,
        (t, r, s): ([f32; 3], [f32; 4], [f32; 3]),
    ) -> usize {
        let mesh = node.mesh().and_then(|m| self.mesh_ids.get(m.index()));
        let name = match (node.name(), mesh) {
            (Some(name), _) => name.to_string(),
            (None, Some(mesh)) => mesh.to_string(),
            (None, None) => format!("node_{}", node.index()),
        };
        self.out.push(ModelNode {
            name,
            parent,
            bone,
            translation: t.into(),
            rotation: Quat::from_array(r),
            scale: s.into(),
            mesh: mesh.map(|m| m.to_string()),
        });
        self.out.len() - 1
    }
}

#[cfg(test)]
#[path = "nodes_tests.rs"]
mod tests;
