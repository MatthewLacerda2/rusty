use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::prefab::diff::masked;
use crate::prefab::live::shift_baseline;
use rusty_asset::{AssetRef, Imported, MeshRevision, SubMesh};
use rusty_core::fs::written::Stale;
use rusty_scene::rehydrate::install_sub_mesh;
use rusty_scene::Scene;

mod shared;
#[cfg(test)]
mod shared_tests;

/// What a skinned mesh does: `Keep` for a setting that never changes it, since re-binding its rig
/// would touch its bones for nothing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Rig {
    Keep,
    Rebuild,
}

pub fn reimport_model(scene: &mut Scene, path: &str, rig: Rig) -> usize {
    reimport_model_ids(scene, path, rig).len()
}

pub fn reimport_model_ids(scene: &mut Scene, path: &str, rig: Rig) -> Vec<u32> {
    reimport_model_except(scene, path, rig, &BTreeSet::new())
}

pub fn reimport_model_except(
    scene: &mut Scene,
    path: &str,
    rig: Rig,
    skip: &BTreeSet<u32>,
) -> Vec<u32> {
    reimport_where(scene, rig, skip, |file| file == path)
}

pub fn reimport_written(scene: &mut Scene, stale: &Stale, skip: &BTreeSet<u32>) -> usize {
    if stale.is_empty() {
        return 0;
    }
    let mut named: BTreeMap<String, bool> = BTreeMap::new();
    let rebuilt = reimport_where(scene, Rig::Rebuild, skip, |file| {
        *named
            .entry(file.to_string())
            .or_insert_with_key(|file| rusty_asset::humanoid::clips_follow(file, stale))
    });
    rebuilt.len()
}

fn reimport_where(
    scene: &mut Scene,
    rig: Rig,
    skip: &BTreeSet<u32>,
    mut names: impl FnMut(&str) -> bool,
) -> Vec<u32> {
    let mut imported: BTreeMap<String, Option<Imported>> = BTreeMap::new();
    let mut rebuilt = Vec::new();
    for id in scene.world.ids_with_mesh() {
        if skip.contains(&id) {
            continue;
        }
        let Some((reference, was_skinned)) = scene.world.mesh(id).and_then(|m| {
            let reference = AssetRef::parse(m.asset_ref.as_deref()?).ok()?;
            Some((reference, m.skin.is_some()))
        }) else {
            continue;
        };
        if (was_skinned && rig == Rig::Keep) || !names(&reference.path) {
            continue;
        }
        let asset = imported
            .entry(reference.path)
            .or_insert_with_key(|file| rusty_asset::imported(Path::new(file)).ok());
        let sub = asset
            .as_ref()
            .and_then(|a| Some((a.sub_mesh(&reference.sub_object)?, a.revision)));
        let linked = scene.world.prefab_link(id).is_some();
        let before = linked.then(|| scene.world.reference_document(id)).flatten();
        if sub.is_some_and(|sub| reimport_entity_mesh(scene, id, sub, rig)) {
            // A linked prefab entity's refit is the model's change, never an override.
            if let Some(before) = before {
                shift_baseline(&mut scene.world, id, &masked(&before));
            }
            rebuilt.push(id);
        }
    }
    // A setting that keeps rigs never moves a vertex, and a shared mesh holds only positions and
    // triangles.
    if rig == Rig::Rebuild {
        shared::reimport_shared(scene, skip, &mut names, &mut imported);
    }
    rebuilt
}

fn reimport_entity_mesh(
    scene: &mut Scene,
    id: u32,
    (sub, revision): (&SubMesh, MeshRevision),
    rig: Rig,
) -> bool {
    let was_skinned = scene.world.mesh(id).is_some_and(|m| m.skin.is_some());
    let skinned = was_skinned || sub.skin.is_some();
    if skinned && rig == Rig::Keep {
        return false;
    }
    let Some(bounds) = sub.bounds() else {
        return false;
    };
    // Taken before the rig is replaced, and re-bound to the new one by name.
    let hung = skinned.then(|| scene.take_skeleton(id));
    if let Some(mut mesh) = scene.world.mesh_mut(id) {
        install_sub_mesh(&mut mesh, sub, revision);
    }
    if let Some(hung) = hung {
        scene.restore_skeleton(id, hung);
    }
    if let Some(mut collider) = scene.world.collider_mut(id) {
        rusty_scene::rehydrate::refit_collider(&mut collider, Some(bounds));
    }
    true
}
