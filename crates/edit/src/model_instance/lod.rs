//! A mesh node named `<base>_LOD<n>` joins the group of its siblings sharing its base and Blender's
//! duplicate suffix: `Crate_LOD0` and `Crate_LOD1` are one prop, and an Alt+D copy `Crate_LOD0.001`
//! / `Crate_LOD1.001` on the same meshes is a second one beside it.

use glam::Mat4;

use crate::lod_instance::{lod0_size, set_levels};
use rusty_asset::lod::{by_level, node_lod};
use rusty_asset::{ImportedAsset, ModelNode};
use rusty_scene::Scene;

#[derive(Default)]
pub(super) struct Lods {
    groups: Vec<Group>,
}

struct Group {
    holder: u32,
    name: String,
    entity: u32,
    first: Mat4,
    members: Vec<(u32, u32, String)>,
}

pub(super) fn group_name(node: &ModelNode) -> Option<String> {
    node.mesh.as_ref()?;
    let (base, _, dup) = node_lod(&node.name)?;
    Some(format!("{base}{dup}"))
}

impl Lods {
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

    pub(super) fn add(&mut self, group: u32, node: &ModelNode, id: u32) {
        let level = node_lod(&node.name).map_or(0, |(_, level, _)| level);
        let mesh = node.mesh.clone().unwrap_or_default();
        if let Some(g) = self.groups.iter_mut().find(|g| g.entity == group) {
            g.members.push((level, id, mesh));
        }
    }

    pub(super) fn finish(self, scene: &mut Scene, asset: &ImportedAsset) {
        for group in self.groups {
            let members = group.members.into_iter();
            let levels = by_level(
                members
                    .map(|(level, id, mesh)| (level, (id, mesh)))
                    .collect(),
            );
            let lod0 = levels.first().into_iter().flatten();
            let size = lod0_size(asset, lod0.map(|(_, mesh)| mesh.as_str()));
            let ids = levels.iter().map(|l| l.iter().map(|(id, _)| *id).collect());
            set_levels(scene, group.entity, ids.collect(), size);
        }
    }
}
