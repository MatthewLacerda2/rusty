//! src/scene/undo/restore.rs — put a step's recorded state back, and record what
//! it replaces as the step that redoes it (#1631).
//!
//! An entity recorded as `None` (created by the step's edit) is despawned; one
//! recorded with a document is put back under its id, in its old slot of the
//! insertion order. Its mesh takes the live geometry when the live mesh comes from
//! the same primitive or asset (the common case: a move, a value, a rename), and is
//! rebuilt from its reference otherwise, as a load rebuilds it.

use std::collections::{BTreeMap, BTreeSet};

use crate::ecs::{Recorded, World};
use crate::scene::Scene;

use super::level::SceneLevel;
use super::Step;

/// Apply `step` to `scene` with the journal off, returning its inverse: the same
/// objects' state as it was just before, under the same name.
pub(super) fn apply(scene: &mut Scene, step: Step) -> Step {
    scene.world.journal_pause();
    let inverse = Step {
        name: step.name.clone(),
        next_id: scene.world.next_id(),
        entities: step
            .entities
            .keys()
            .map(|&id| (id, scene.world.record(id)))
            .collect(),
        level: step.level.as_ref().map(|_| SceneLevel::capture(scene)),
    };
    restore_entities(&mut scene.world, step.entities);
    scene.world.restore_next_id(step.next_id);
    if let Some(level) = step.level {
        level.restore(scene);
    }
    inverse
}

/// Despawn what the step created, then put back every recorded entity.
fn restore_entities(world: &mut World, entities: BTreeMap<u32, Option<Recorded>>) {
    let created: BTreeSet<u32> = entities
        .iter()
        .filter(|(id, before)| before.is_none() && world.contains(**id))
        .map(|(&id, _)| id)
        .collect();
    world.despawn_all(&created);
    for recorded in entities.into_values().flatten() {
        let Recorded { mut doc, seq } = recorded;
        let live = world.mesh_mut(doc.id);
        match (&mut doc.mesh, live) {
            (Some(mesh), Some(mut live)) if mesh.same_source(&live) => {
                mesh.adopt_geometry(&mut live);
            }
            (Some(_), _) => crate::scene::rehydrate::rehydrate_entity_mesh(&mut doc),
            (None, _) => {}
        }
        world.restore_entity(doc, seq);
    }
}
