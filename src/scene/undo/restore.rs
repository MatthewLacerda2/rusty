//! src/scene/undo/restore.rs — put a step's recorded state back, and record what
//! it replaces as the step that redoes it (#1631).
//!
//! An entity recorded as `None` (created by the step's edit) is despawned; one
//! recorded with a document is put back under its id, in its old slot of the
//! insertion order. Its mesh takes the live geometry when the live mesh comes from
//! the same primitive or asset (the common case: a move, a value, a rename), and is
//! rebuilt from its reference otherwise, as a load rebuilds it. A Mesh-shaped
//! collider's shared mesh (#1657), recorded without its geometry too, is put back
//! the same way.

use std::collections::{BTreeMap, BTreeSet};

use crate::components::{ColliderComponent, ColliderShape, Entity};
use crate::ecs::{Recorded, World};
use crate::scene::{Despawn, Scene};

use super::{side::Side, Step};

/// Apply `step` to `scene` with the journal off, returning its inverse: the same
/// objects' state as it was just before, under the same name.
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
    inverse
}

/// Despawn what the step created, through the scene's one door so its own
/// references (the selection, lightmap entries, owned decals, shader overrides,
/// …) go too,
/// then put back every recorded entity, binding the skeleton of each whose mesh
/// was rebuilt (a rebuild leaves the bones unbound, as a load does). An undone create's id stays retired: the
/// allocator never goes back (Unity never hands an undone object's id out
/// again), and a Redo re-creates the entity under that same id.
fn restore_entities(scene: &mut Scene, entities: BTreeMap<u32, Option<Recorded>>) {
    let created: BTreeSet<u32> = entities
        .iter()
        .filter(|(id, before)| before.is_none() && scene.world.contains(**id))
        .map(|(&id, _)| id)
        .collect();
    scene.despawn_batch(&created, Despawn::EditMode);
    let world = &mut scene.world;
    let mut rebuilt = Vec::new();
    for recorded in entities.into_values().flatten() {
        let Recorded { mut doc, seq } = recorded;
        let live = world.mesh_mut(doc.id);
        match (&mut doc.mesh, live) {
            (Some(mesh), Some(mut live)) if mesh.same_source(&live) => {
                mesh.adopt_geometry(&mut live);
            }
            (Some(_), _) => {
                crate::scene::rehydrate::rehydrate_entity_mesh(&mut doc);
                rebuilt.push(doc.id);
            }
            (None, _) => {}
        }
        if doc.mesh.is_none() {
            restore_shared_mesh(world, &mut doc);
        }
        world.restore_entity(doc, seq);
    }
    // After every entity is back, so each skeleton finds its restored bones.
    for owner in rebuilt {
        scene.sync_skeleton(owner);
    }
}

/// Give `doc`'s collider's shared mesh (recorded without geometry) the live one's
/// when it comes from the same source, and rebuild it from its reference otherwise.
/// Only a Mesh-shaped collider's: another shape never reads it.
fn restore_shared_mesh(world: &mut World, doc: &mut Entity) {
    let mesh_shaped = |c: &&mut ColliderComponent| matches!(c.shape, ColliderShape::Mesh { .. });
    let collider = doc.collider.as_mut().filter(mesh_shaped);
    let Some(shared) = collider.and_then(|c| c.shared_mesh.as_mut()) else {
        return;
    };
    if let Some(mut live) = world.collider_mut(doc.id) {
        if let Some(live) = live.shared_mesh.as_mut().filter(|l| shared.same_source(l)) {
            shared.adopt_geometry(live);
            return;
        }
    }
    crate::scene::rehydrate::rehydrate_entity_mesh(doc);
}
