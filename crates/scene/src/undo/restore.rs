use std::collections::{BTreeMap, BTreeSet};

use crate::rehydrate::{adopt_or_rehydrate, Imports, Keep};
use crate::{Despawn, Scene};
use rusty_ecs::Recorded;

use super::{side::Side, Step};

pub(super) fn apply(scene: &mut Scene, step: Step) -> Step {
    scene.world.journal_pause();
    let inverse = Step {
        name: step.name.clone(),
        entities: step
            .entities
            .keys()
            .map(|&id| (id, scene.world.record(id)))
            .collect(),
        level: step.level.as_ref().map(|delta| delta.inverse(scene)),
        side: Side::of(scene, step.entities.keys().copied()),
    };
    restore_entities(scene, step.entities);
    step.side.put_back(scene);
    if let Some(level) = step.level {
        level.restore(scene);
    }
    // After the level: a library mesh's users take its put-back data.
    scene.sync_scene_meshes();
    // Once per step, not per entity: every change cursor rebuilds.
    scene.world.rewind_changes();
    inverse
}

fn restore_entities(scene: &mut Scene, entities: BTreeMap<u32, Option<Recorded>>) {
    let created: BTreeSet<u32> = entities
        .iter()
        .filter(|(id, before)| before.is_none() && scene.world.contains(**id))
        .map(|(&id, _)| id)
        .collect();
    // Through the one despawn door, so the selection, lightmap entries and owned decals go too.
    scene.despawn_batch(&created, Despawn::EditMode);
    let world = &mut scene.world;
    let mut rebuilt = Vec::new();
    let mut imports = Imports::default();
    for recorded in entities.into_values().flatten() {
        let Recorded { mut doc, seq } = recorded;
        if adopt_or_rehydrate(world, &mut doc, Keep::SameSource, &mut imports) {
            rebuilt.push(doc.id);
        }
        world.restore_entity(doc, seq);
    }
    // Once every entity is back, so each skeleton finds its restored bones.
    for owner in rebuilt {
        scene.sync_skeleton(owner);
    }
}
