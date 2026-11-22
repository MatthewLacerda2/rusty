use glam::{Mat4, Vec3};

use super::build::{build, Node};
use super::mesh::{MeshTree, Tri, TriUv};
use crate::frustum::transform_aabb;
use crate::scene::{MeshHandle, RenderObject};
use rusty_components::RenderFace;

pub(crate) const NONE: u32 = u32::MAX;

/// How a run's triangles are hit, as the material's cull mode draws them.
pub(crate) mod face {
    pub const SKIP: u32 = 0;
    pub const FRONT: u32 = 1;
    pub const BACK: u32 = 2;
    pub const BOTH: u32 = 3;
}

/// A static object in the top tree: its world-to-local rows, its mesh's root node, its
/// slot of the object table, and its runs' [`face`] codes in `faces`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Instance {
    pub to_local: [[f32; 4]; 3],
    pub root: u32,
    pub slot: u32,
    pub faces: u32,
    pub runs: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Header {
    pub top: u32,
    pub instances: u32,
    pub pad: [u32; 2],
}

/// The whole tree as the GPU holds it: every mesh's nodes, then the top's.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Packed {
    pub header: Header,
    pub nodes: Vec<Node>,
    pub tris: Vec<Tri>,
    pub uvs: Vec<TriUv>,
    pub instances: Vec<Instance>,
    pub faces: Vec<u32>,
}

/// One traced object, before packing.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Placed {
    pub mesh: MeshHandle,
    pub world: Mat4,
    pub slot: u32,
    pub faces: Vec<u32>,
}

impl Placed {
    pub(crate) fn of(object: &RenderObject) -> Option<Self> {
        let faces: Vec<u32> = object.slots.iter().map(slot_face).collect();
        let traced = faces.iter().any(|&f| f != face::SKIP);
        let inverse = object.world.inverse();
        (traced && inverse.is_finite()).then(|| Self {
            mesh: object.mesh,
            world: object.world,
            slot: object.slot,
            faces,
        })
    }
}

fn slot_face(slot: &crate::scene::SlotDraw) -> u32 {
    if slot.transparent || slot.clip.is_some() {
        return face::SKIP;
    }
    match slot.face {
        RenderFace::Front => face::FRONT,
        RenderFace::Back => face::BACK,
        RenderFace::Both => face::BOTH,
    }
}

/// Packs `meshes` (each with its root's node) and the top tree over `placed`.
pub(crate) fn pack<'a>(
    meshes: impl Iterator<Item = (MeshHandle, &'a MeshTree)>,
    placed: &[Placed],
) -> (Packed, u32) {
    let mut p = Packed::default();
    let mut roots = std::collections::BTreeMap::new();
    for (handle, tree) in meshes {
        let (node_base, tri_base) = (p.nodes.len() as u32, p.tris.len() as u32);
        let rebased = tree.nodes.iter().map(|&n| Node {
            first: n.first + if n.count == 0 { node_base } else { tri_base },
            ..n
        });
        p.nodes.extend(rebased);
        p.tris.extend_from_slice(&tree.tris);
        p.uvs.extend_from_slice(&tree.uvs);
        let root = tree.nodes.first().map(|n| (node_base, *n));
        roots.insert(handle, root);
    }
    let geometry = p.nodes.len() as u32;
    let (mut instances, mut bounds) = (Vec::new(), Vec::new());
    for o in placed {
        let Some(&Some((root, node))) = roots.get(&o.mesh) else {
            continue;
        };
        let (lo, hi) = (Vec3::from_array(node.lo), Vec3::from_array(node.hi));
        bounds.push(transform_aabb(lo, hi, o.world));
        let rows = o.world.inverse().transpose();
        instances.push(Instance {
            to_local: [rows.x_axis, rows.y_axis, rows.z_axis].map(|r| r.to_array()),
            root,
            slot: o.slot,
            faces: p.faces.len() as u32,
            runs: o.faces.len() as u32,
        });
        p.faces.extend_from_slice(&o.faces);
    }
    let (top, order) = build(&bounds);
    let base = p.nodes.len() as u32;
    let rebased = top.iter().map(|&n| Node {
        first: n.first + if n.count == 0 { base } else { 0 },
        ..n
    });
    p.nodes.extend(rebased);
    p.instances = order.iter().map(|&i| instances[i as usize]).collect();
    let top = if p.instances.is_empty() { NONE } else { base };
    p.header = Header {
        top,
        instances: p.instances.len() as u32,
        pad: [0; 2],
    };
    (p, geometry)
}
