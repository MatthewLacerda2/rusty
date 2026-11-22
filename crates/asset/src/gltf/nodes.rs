//! Nodes come depth-first from the default scene (else the first, else every parentless
//! node), each after its parent.
//!
//! A joint of a skin some kept mesh uses is left out: it spawns as that skinned mesh's
//! bone child. What a joint holds hangs from the bone (Unity keeps a gun under `Hand`): it
//! is kept with the skinned node as parent and the bone named ([`NodeBone`]), walked after
//! the main tree. A node reached twice is kept the first time only.

use glam::{Mat4, Quat};

use crate::model_node::{ModelNode, NodeBone};

pub fn model_nodes<'a>(document: &'a gltf::Document, mesh_ids: &'a [&'a str]) -> Vec<ModelNode> {
    let roots = roots(document);
    let mut walk = Walk {
        mesh_ids,
        bone: bones(document, &roots),
        global: globals(document, &roots),
        seen: vec![false; document.nodes().len()],
        kept: vec![None; document.nodes().len()],
        held: Vec::new(),
        out: Vec::new(),
    };
    for root in &roots {
        walk.node(root, None, None);
    }
    while !walk.held.is_empty() {
        for (held, owner, name, joint) in std::mem::take(&mut walk.held) {
            let parent = walk.kept[owner];
            // Relative to the skinned node, as `gltf::skin` builds `SkinData::bind_global`.
            let rest = walk.global[owner].inverse() * walk.global[joint];
            let bone = Some(NodeBone { name, rest });
            match held {
                Held::Child(node) => walk.node(&node, parent, bone),
                Held::Mesh(node) => {
                    let at_bone = ([0.0; 3], Quat::IDENTITY.to_array(), [1.0; 3]);
                    walk.push(&node, parent, bone, at_bone);
                }
            }
        }
    }
    walk.out
}

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

fn globals(document: &gltf::Document, roots: &[gltf::Node]) -> Vec<Mat4> {
    let mut out = vec![None; document.nodes().len()];
    let mut stack: Vec<(gltf::Node, Mat4)> =
        roots.iter().map(|r| (r.clone(), Mat4::IDENTITY)).collect();
    while let Some((node, parent)) = stack.pop() {
        if out[node.index()].is_some() {
            continue;
        }
        let global = parent * Mat4::from_cols_array_2d(&node.transform().matrix());
        out[node.index()] = Some(global);
        stack.extend(node.children().map(|c| (c, global)));
    }
    out.into_iter()
        .map(|g| g.unwrap_or(Mat4::IDENTITY))
        .collect()
}

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
            // As `gltf::skin` names it: `edit::model_instance` finds the bone by it.
            let name = j
                .name()
                .map_or_else(|| format!("joint_{slot}"), str::to_string);
            out[j.index()].get_or_insert((node.index(), name));
        }
    }
    out
}

enum Held<'a> {
    Child(gltf::Node<'a>),
    Mesh(gltf::Node<'a>),
}

struct Walk<'a> {
    mesh_ids: &'a [&'a str],
    bone: Vec<Option<(usize, String)>>,
    global: Vec<Mat4>,
    seen: Vec<bool>,
    kept: Vec<Option<usize>>,
    held: Vec<(Held<'a>, usize, String, usize)>,
    out: Vec<ModelNode>,
}

impl<'a> Walk<'a> {
    fn node(&mut self, node: &gltf::Node<'a>, parent: Option<usize>, bone: Option<NodeBone>) {
        if std::mem::replace(&mut self.seen[node.index()], true) {
            return;
        }
        if let Some((owner, name)) = self.bone[node.index()].clone() {
            let joint = node.index();
            if node.mesh().is_some() {
                self.held
                    .push((Held::Mesh(node.clone()), owner, name.clone(), joint));
            }
            for child in node.children() {
                match self.bone[child.index()].is_some() {
                    true => self.node(&child, parent, None),
                    false => self
                        .held
                        .push((Held::Child(child), owner, name.clone(), joint)),
                }
            }
            return;
        }
        let (t, r, s) = node.transform().decomposed();
        let index = self.push(node, parent, bone, (t, r, s));
        self.kept[node.index()] = Some(index);
        for child in node.children() {
            self.node(&child, Some(index), None);
        }
    }

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
