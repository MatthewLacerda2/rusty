//! A Play load's renumbering: survivors keep their ids, so an incoming entity holding one moves
//! to a fresh id, and every field outside the World naming entities follows it.

use std::collections::{BTreeMap, BTreeSet};

use crate::lighting::lightmap_set::LightmapSet;
use crate::serialize::SceneData;
use crate::Scene;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Moves {
    moved: BTreeMap<u32, u32>,
    survivors: BTreeSet<u32>,
    next: u32,
}

impl Moves {
    /// The one place a moved id is chosen: past the document's next id, every incoming id and every
    /// survivor's. In document order, so the result is a pure function of the inputs.
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

    pub fn to(&self, id: u32) -> u32 {
        self.moved.get(&id).copied().unwrap_or(id)
    }

    pub fn is_empty(&self) -> bool {
        self.moved.is_empty()
    }

    pub fn next_id(&self) -> u32 {
        self.next
    }

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

#[derive(Clone, Copy)]
pub(crate) enum Pass {
    Document(fn(&mut SceneData, &Moves)),
    Sidecar(fn(&mut Scene, &Moves)),
}

/// `state::renumber_tests` fails a saved field keyed by entity that is missing here.
pub(crate) const RENUMBERED: &[(&str, Pass)] = &[
    ("selected_entity_id", Pass::Document(renumber_selection)),
    ("lightmaps", Pass::Document(renumber_lightmaps)),
    ("occlusion", Pass::Sidecar(renumber_occlusion)),
];

fn renumber_selection(data: &mut SceneData, moves: &Moves) {
    let id = data.selected_entity_id.map(|id| moves.to(id));
    data.selected_entity_id = id.filter(|id| !moves.survivors.contains(id));
}

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
    // What is left under a survivor's id names an entity deleted since the bake, not the
    // survivor.
    data.lightmaps.forget(&moves.survivors);
}

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
