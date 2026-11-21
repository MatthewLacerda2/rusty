//! A Play load's renumbering: survivors keep their ids, so an incoming entity
//! whose id a survivor holds moves to a fresh one ([`Moves`]), and every field
//! that names entities by id outside the World follows it ([`RENUMBERED`]).
//!
//! What the **document** carries by id (the selection, the lightmaps) is renamed in
//! the load's document pass, with the entities (`rusty_edit::load::make_room`), so
//! the World and those fields never disagree while the document is applied: a
//! despawn during the apply (a generated hitbox whose bone is gone) clears what
//! names the despawned entity by its new id. What a **separate file** carries (the
//! baked PVS) is renamed once that file is in ([`Scene::renumber_sidecars`]). The
//! state inventory holds every saved field keyed by entity to one of the two
//! (`state::renumber_tests`).

use std::collections::{BTreeMap, BTreeSet};

use crate::lighting::lightmap_set::LightmapSet;
use crate::serialize::SceneData;
use crate::Scene;

/// A load's renumbering: the survivors, and the incoming entities moved out of
/// their way, old id → new. Made only by [`Moves::out_of_the_way`], the one place
/// a moved entity's id is chosen, so every new id is fresh: past the document's
/// next id, every incoming id and every survivor's.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Moves {
    moved: BTreeMap<u32, u32>,
    survivors: BTreeSet<u32>,
    next: u32,
}

impl Moves {
    /// Move every incoming entity of `data` whose id `survivors` hold to a fresh
    /// id, in document order, so the result is a pure function of the inputs.
    /// Nothing in `data` changes: the document pass applies the moves.
    pub fn out_of_the_way(data: &SceneData, survivors: &BTreeSet<u32>) -> Self {
        let ids = data.entities.iter().map(|e| e.id);
        let past = ids.chain(survivors.iter().copied()).map(|id| id + 1);
        let mut next = past.fold(data.next_entity_id, u32::max);
        let mut moved = BTreeMap::new();
        for entity in &data.entities {
            if survivors.contains(&entity.id) {
                moved.insert(entity.id, next);
                next += 1;
            }
        }
        let survivors = survivors.clone();
        Self {
            moved,
            survivors,
            next,
        }
    }

    /// Where `id` is after the moves: its new id, or itself if it did not move.
    pub fn to(&self, id: u32) -> u32 {
        self.moved.get(&id).copied().unwrap_or(id)
    }

    /// Whether no entity moved.
    pub fn is_empty(&self) -> bool {
        self.moved.is_empty()
    }

    /// The document's next free id once the moved entities took theirs.
    pub fn next_id(&self) -> u32 {
        self.next
    }

    /// Rename what the document carries by id ([`Pass::Document`]) and drop the
    /// survivors' ids from it: a survivor carries none of the level's id-keyed
    /// data across. No survivors, no change.
    pub fn rename_document(&self, data: &mut SceneData) {
        if self.survivors.is_empty() {
            return;
        }
        for (_, pass) in RENUMBERED {
            if let Pass::Document(follow) = pass {
                follow(data, self);
            }
        }
    }
}

impl Scene {
    /// Rename what the load's separate files carry by id ([`Pass::Sidecar`]),
    /// once they are in. No moves, no change.
    pub fn renumber_sidecars(&mut self, moves: &Moves) {
        if moves.is_empty() {
            return;
        }
        for (_, pass) in RENUMBERED {
            if let Pass::Sidecar(follow) = pass {
                follow(self, moves);
            }
        }
    }
}

/// When a field keyed by entity follows a load's renumbering.
#[derive(Clone, Copy)]
pub(crate) enum Pass {
    /// The document carries it: renamed in the document pass, with the entities.
    Document(fn(&mut SceneData, &Moves)),
    /// A separate file carries it: renamed once that file is in.
    Sidecar(fn(&mut Scene, &Moves)),
}

/// Every field of `Scene` outside the World that names entity ids loaded from the
/// scene's files, with the pass that renames it. The state inventory's test fails
/// a saved field keyed by entity that is not here.
pub(crate) const RENUMBERED: &[(&str, Pass)] = &[
    ("selected_entity_id", Pass::Document(renumber_selection)),
    ("lightmaps", Pass::Document(renumber_lightmaps)),
    ("occlusion", Pass::Sidecar(renumber_occlusion)),
];

/// The selection follows its entity; one naming a survivor's id names no
/// incoming entity, so it goes.
fn renumber_selection(data: &mut SceneData, moves: &Moves) {
    let id = data.selected_entity_id.map(|id| moves.to(id));
    data.selected_entity_id = id.filter(|id| !moves.survivors.contains(id));
}

/// The lightmaps follow their entities, and then forget every survivor's id:
/// what is left under one names an entity deleted since the bake (a destroyed
/// light still reads as baked), which a survivor holding that id must not inherit.
fn renumber_lightmaps(data: &mut SceneData, moves: &Moves) {
    if !moves.is_empty() {
        data.lightmaps = data.lightmaps.remap(moves).unwrap_or_else(|| {
            log::warn!(
                "[Lighting] the lightmaps' ids collide after the load: the level draws \
                 unlightmapped until the next bake"
            );
            LightmapSet::default()
        });
    }
    data.lightmaps.forget(&moves.survivors);
}

/// The baked PVS was baked against the file's ids: rename the objects a load
/// moved, so a carried entity never turns the level's culling off.
fn renumber_occlusion(scene: &mut Scene, moves: &Moves) {
    let Some(data) = scene.occlusion.as_deref() else {
        return;
    };
    let remapped = data.remap(moves);
    if remapped.is_none() {
        log::warn!(
            "[Occlusion] the bake's ids collide after the load: nothing is occlusion-culled"
        );
    }
    *scene.occlusion = remapped.map(std::sync::Arc::new);
}
