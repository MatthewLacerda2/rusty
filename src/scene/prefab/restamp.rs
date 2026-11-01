//! src/scene/prefab/restamp.rs — rebuilding many linked instances at once: every
//! one in a loaded scene, or every one of a model that was just re-imported
//! (#1637, Unity's model prefab). Each source is read once per call, however many
//! instances it has, so a Dust2-sized level holding hundreds of one model builds
//! that model's prefab once.

use std::collections::{BTreeMap, BTreeSet};

use crate::scene::prefab::instance::instance_members;
use crate::scene::prefab::link::{finish_rebuild, link_source, rebuild_instance};
use crate::scene::prefab::live::sync_overrides;
use crate::scene::prefab::source::{is_model_source, node_paths, read_source};
use crate::scene::{PrefabData, Scene};

/// Run `refit` (a re-import of the models `names` accepts, #1637) as those models'
/// change on every instance linked to one: each instance's edits are taken as
/// overrides against the model as it was, then `refit` runs, then each instance is
/// rebuilt from the model as it imports now with those overrides on top. So a
/// part keeps a hand edit and takes everything else (a Scale Factor's offsets, a
/// moved node) from the file. It writes entities but is no edit: unrecorded, as a
/// bake is, when an instance is there to rebuild. `refit` is handed the instances'
/// static meshed parts, which the rebuild re-imports anyway, for it to skip; a
/// skinned part is left to it, since its rig and bones are the refit's to move.
/// The rebuild then gives each instance the model's node tree: a node the model
/// gained is spawned, one it lost is destroyed (`structure`, #1665).
pub fn restamp_models<R>(
    scene: &mut Scene,
    names: impl Fn(&str) -> bool,
    refit: impl FnOnce(&mut Scene, &BTreeSet<u32>) -> R,
) -> R {
    let roots = instance_roots(scene, |s| is_model_source(s) && names(s));
    if roots.is_empty() {
        return refit(scene, &BTreeSet::new());
    }
    scene.unrecorded(|scene| {
        let mut skip = BTreeSet::new();
        let mut nodes: BTreeMap<String, Option<BTreeSet<String>>> = BTreeMap::new();
        for &root in &roots {
            let Some(ids) = instance_members(scene, root) else {
                continue;
            };
            sync_overrides(&mut scene.world, &ids);
            // Only a part the rebuild will reach is skipped: one whose node the
            // model still has. A gone part (or every part of a model that no longer
            // reads) is left to `refit`, or it would keep stale geometry.
            let Some(Some(kept)) = link_source(scene, root)
                .ok()
                .map(|s| nodes.entry(s).or_insert_with_key(|s| node_set(s)).as_ref())
            else {
                continue;
            };
            let rebuilt = |id: &u32| {
                let static_mesh = scene.world.mesh(*id).is_some_and(|m| m.skin.is_none());
                let node = scene.world.prefab_link(*id).and_then(|l| l.node.clone());
                static_mesh && node.is_some_and(|n| kept.contains(&n))
            };
            skip.extend(ids.iter().copied().filter(rebuilt));
        }
        let out = refit(scene, &skip);
        propagate_roots(scene, roots);
        out
    })
}

/// The node paths of the model at `source` as it imports now, `None` when it
/// can't be read.
fn node_set(source: &str) -> Option<BTreeSet<String>> {
    let prefab = read_source(source).ok()?;
    Some(node_paths(&prefab).into_values().collect())
}

/// The instance roots (a linked entity with `local_id == 0`) whose source `pick`
/// accepts.
pub(super) fn instance_roots(scene: &Scene, pick: impl Fn(&str) -> bool) -> Vec<u32> {
    let is_root = |id: u32| {
        let link = scene.world.prefab_link(id);
        link.is_some_and(|l| l.local_id == 0 && pick(&l.source))
    };
    let ids = scene.world.ids_with_prefab_link().into_iter();
    ids.filter(|&id| is_root(id)).collect()
}

/// Propagate each of `roots`, reading each source once; a root whose source can't
/// be read is skipped. The whole-scene passes run once, after the last instance.
pub(super) fn propagate_roots(scene: &mut Scene, roots: Vec<u32>) {
    let mut sources: BTreeMap<String, Option<PrefabData>> = BTreeMap::new();
    let mut rebuilt = false;
    for root in roots {
        let Ok(source) = link_source(scene, root) else {
            continue;
        };
        let prefab = sources
            .entry(source.clone())
            .or_insert_with_key(|s| read_source(s).ok());
        if let Some(prefab) = prefab.as_ref() {
            rebuilt |= rebuild_instance(scene, root, &source, prefab, true).is_ok();
        }
    }
    if rebuilt {
        finish_rebuild(scene);
    }
}
