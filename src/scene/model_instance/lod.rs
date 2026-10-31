//! The LODGroups of a whole-model spawn (#472, #1083), grouped per node.
//!
//! A mesh node named `<base>_LOD<n>` (`asset::lod::node_lod`) joins the group of
//! its siblings sharing its base and Blender's duplicate suffix: so `Crate_LOD0`
//! and `Crate_LOD1` are one prop, and an Alt+D copy `Crate_LOD0.001` /
//! `Crate_LOD1.001` on the same meshes is a second one beside it. The group
//! entity sits where its first member's node does, and each member under it
//! where its own node puts it relative to that one.

use glam::Mat4;

use crate::asset::lod::node_lod;
use crate::asset::{ImportedAsset, ModelNode};
use crate::scene::lod_instance::{lod0_size, set_levels};
use crate::scene::Scene;

/// The groups spawned so far, finished once every node is.
#[derive(Default)]
pub(super) struct Lods {
    groups: Vec<Group>,
}

struct Group {
    /// The entity its members hang under, and its name there.
    holder: u32,
    name: String,
    entity: u32,
    /// Its first member's local transform under `holder`.
    first: Mat4,
    /// Each member: its level, entity and mesh.
    members: Vec<(u32, u32, String)>,
}

/// The name of the group mesh node `node` joins, its base and duplicate suffix
/// (`Crate`, `Crate.001`); `None` for a node of no group.
pub(super) fn group_name(node: &ModelNode) -> Option<String> {
    node.mesh.as_ref()?;
    let (base, _, dup) = node_lod(&node.name)?;
    Some(format!("{base}{dup}"))
}

impl Lods {
    /// The group `node` belongs to under `holder` — made where `node` sits
    /// (`held · node.local()`) when it is the first — and the node's local
    /// transform within it; `None` for a node of no group.
    pub(super) fn group_for(
        &mut self,
        scene: &mut Scene,
        node: &ModelNode,
        holder: u32,
        held: Mat4,
    ) -> Option<(u32, Mat4)> {
        let name = group_name(node)?;
        let local = held * node.local();
        let found = self
            .groups
            .iter()
            .find(|g| g.holder == holder && g.name == name);
        if let Some(group) = found {
            return Some((group.entity, group.first.inverse() * local));
        }
        let entity = scene.add_entity(name.clone());
        super::set_matrix(scene, entity, local);
        let _ = scene.set_parent(entity, Some(holder));
        self.groups.push(Group {
            holder,
            name,
            entity,
            first: local,
            members: Vec::new(),
        });
        Some((entity, Mat4::IDENTITY))
    }

    /// Record `id`, spawned for `node`, as a member of group entity `group`.
    pub(super) fn add(&mut self, group: u32, node: &ModelNode, id: u32) {
        let level = node_lod(&node.name).map_or(0, |(_, level, _)| level);
        let mesh = node.mesh.clone().unwrap_or_default();
        if let Some(g) = self.groups.iter_mut().find(|g| g.entity == group) {
            g.members.push((level, id, mesh));
        }
    }

    /// Give every group its `LODGroup`: levels finest first, sized by LOD0's meshes.
    pub(super) fn finish(self, scene: &mut Scene, asset: &ImportedAsset) {
        for mut group in self.groups {
            group.members.sort_by_key(|(level, _, _)| *level);
            let mut levels: Vec<(u32, Vec<u32>)> = Vec::new();
            for &(level, id, _) in &group.members {
                match levels.last_mut() {
                    Some((l, ids)) if *l == level => ids.push(id),
                    _ => levels.push((level, vec![id])),
                }
            }
            let finest = levels.first().map(|(l, _)| *l);
            let lod0 = group.members.iter().filter(|(l, _, _)| Some(*l) == finest);
            let size = lod0_size(asset, lod0.map(|(_, _, mesh)| mesh.as_str()));
            let levels = levels.into_iter().map(|(_, ids)| ids).collect();
            set_levels(scene, group.entity, levels, size);
        }
    }
}
