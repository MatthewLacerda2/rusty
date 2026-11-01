//! src/scene/rehydrate/reimport.rs — re-import the meshes already in a scene when
//! their model changed: an import setting (Generate Lightmap UVs, #831; the Scale
//! Factor, #1398) or the file itself (#1242). Each model file is looked up once per
//! call however many instances name it, through its shared import
//! (`asset::imported`, #857), so a Dust2-sized level with hundreds of one prop
//! parses and unwraps it once per edit, and not at all when nothing changed.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use glam::Vec3;

use super::{install, RehydratedMesh};
use crate::asset::{self, AssetRef, Imported, MeshRevision, SubMesh};
use crate::components::{ColliderComponent, ColliderShape};
use crate::core::fs::written::Stale;
use crate::scene::prefab::diff::masked;
use crate::scene::prefab::live::shift_baseline;
use crate::scene::Scene;

/// What a re-import does with a skinned mesh (one skinned before or after it).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Rig {
    /// Leave it as it is.
    Keep,
    /// Take the new mesh and rig, and re-bind the bones to it.
    Rebuild,
}

/// Re-import every mesh in `scene` instanced from the model at `path`, so a changed
/// import setting shows at once rather than on the next load. `rig` says what a
/// skinned one does: a setting that never touches them (Generate Lightmap UVs, #831)
/// keeps them, since re-binding their rig would touch their bones for nothing; one
/// that does (the Scale Factor, #1398) rebuilds them, their bones re-bound by name.
/// Returns how many were rebuilt.
pub fn reimport_model(scene: &mut Scene, path: &str, rig: Rig) -> usize {
    reimport_model_ids(scene, path, rig).len()
}

/// [`reimport_model`], returning the ids of the entities whose mesh was rebuilt.
pub fn reimport_model_ids(scene: &mut Scene, path: &str, rig: Rig) -> Vec<u32> {
    reimport_model_except(scene, path, rig, &BTreeSet::new())
}

/// [`reimport_model_ids`] leaving out the entities in `skip` (a linked model
/// instance's parts, which its rebuild re-imports, #1637).
pub fn reimport_model_except(
    scene: &mut Scene,
    path: &str,
    rig: Rig,
    skip: &BTreeSet<u32>,
) -> Vec<u32> {
    reimport_where(scene, rig, skip, |file| file == path)
}

/// Re-import every mesh in `scene` whose model file `stale` names: the engine
/// re-wrote it (#1242), and Unity updates every instance of a re-imported model.
/// A skinned one takes the new rig too, its bones re-bound to it by name (#1265). Each
/// distinct file is matched against `stale` once. The entities in `skip` are left
/// out (a linked model instance's parts, which its rebuild re-imports, #1637).
/// Returns how many were rebuilt.
pub fn reimport_written(scene: &mut Scene, stale: &Stale, skip: &BTreeSet<u32>) -> usize {
    if stale.is_empty() {
        return 0;
    }
    let mut named: BTreeMap<String, bool> = BTreeMap::new();
    let rebuilt = reimport_where(scene, Rig::Rebuild, skip, |file| {
        *named
            .entry(file.to_string())
            .or_insert_with_key(|file| stale.matches(file))
    });
    rebuilt.len()
}

/// Re-import every mesh outside `skip` whose reference's file `names` accepts,
/// importing each such file once. Returns the ids of the entities whose mesh was
/// rebuilt.
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
            .or_insert_with_key(|file| asset::imported(Path::new(file)).ok());
        let sub = asset
            .as_ref()
            .and_then(|a| Some((a.sub_mesh(&reference.sub_object)?, a.revision)));
        // A linked prefab entity's refit is the model's, never an override (#1527).
        let linked = scene.world.prefab_link(id).is_some();
        let before = linked.then(|| scene.world.reference_document(id)).flatten();
        if sub.is_some_and(|sub| reimport_entity_mesh(scene, id, sub, rig)) {
            if let Some(before) = before {
                shift_baseline(&mut scene.world, id, &masked(&before));
            }
            rebuilt.push(id);
        }
    }
    rebuilt
}

/// Put `sub` on entity `id`'s mesh, and refit its mesh collider's box to it. A
/// skinned mesh (before or after) takes the new rig, its bones re-bound, when `rig`
/// says so, and is kept otherwise. `revision` is the import's `sub` came from.
/// Returns whether the mesh was rebuilt.
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
    // The bones are noted against the rig they were built from before it is
    // replaced, and re-bound to the new one by name.
    let hung = skinned.then(|| scene.take_skeleton(id));
    if let Some(mut mesh) = scene.world.mesh_mut(id) {
        install(&mut mesh, RehydratedMesh::from_sub_mesh(sub, revision));
    }
    if let Some(hung) = hung {
        scene.restore_skeleton(id, hung);
    }
    // The guard refreshes the world AABB.
    if let Some(mut collider) = scene.world.collider_mut(id) {
        refit(&mut collider, bounds);
    }
    true
}

/// Size `collider`'s box to `(min, max)` when it is a mesh collider, as Unity's
/// `MeshCollider` follows its shared mesh. Any other shape is left as authored.
pub(super) fn refit(collider: &mut ColliderComponent, (min, max): (Vec3, Vec3)) {
    if let ColliderShape::Mesh {
        local_min,
        local_max,
        ..
    } = &mut collider.shape
    {
        (*local_min, *local_max) = (min, max);
    }
}
