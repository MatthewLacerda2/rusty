//! src/ecs/query_tests.rs — the narrow-query ordering contract (#346).
//!
//! `ids_with_*` is a narrow hecs query sorted by `Core::seq`; these tests pin
//! the determinism contract it must honour: matches come back in insertion
//! order, regardless of when the component was attached, and an entity
//! rebuilt in place (`replace_entity`) keeps its slot.

use crate::components::ParticleEmitterComponent;
use crate::ecs::World;

fn emitter() -> ParticleEmitterComponent {
    ParticleEmitterComponent::default()
}

#[test]
fn only_carriers_are_returned() {
    let mut w = World::new();
    let a = w.spawn("a".into());
    let b = w.spawn("b".into());
    let _c = w.spawn("c".into());
    w.set_particles(a, Some(emitter()));
    w.set_particles(b, Some(emitter()));
    assert_eq!(w.ids_with_particles(), vec![a, b]);
}

#[test]
fn order_is_insertion_order_not_attach_order() {
    let mut w = World::new();
    let a = w.spawn("a".into());
    let b = w.spawn("b".into());
    let c = w.spawn("c".into());
    // Attach in reverse spawn order; the query must still yield spawn order.
    w.set_particles(c, Some(emitter()));
    w.set_particles(a, Some(emitter()));
    w.set_particles(b, Some(emitter()));
    assert_eq!(w.ids_with_particles(), vec![a, b, c]);
}

#[test]
fn detach_and_despawn_remove_from_results() {
    let mut w = World::new();
    let a = w.spawn("a".into());
    let b = w.spawn("b".into());
    let c = w.spawn("c".into());
    for id in [a, b, c] {
        w.set_particles(id, Some(emitter()));
    }
    w.set_particles(b, None);
    assert_eq!(w.ids_with_particles(), vec![a, c]);
    w.despawn(a);
    assert_eq!(w.ids_with_particles(), vec![c]);
}

#[test]
fn replace_entity_keeps_its_insertion_slot() {
    let mut w = World::new();
    let a = w.spawn("a".into());
    let b = w.spawn("b".into());
    let c = w.spawn("c".into());
    for id in [a, b, c] {
        w.set_particles(id, Some(emitter()));
    }
    // Rebuild the middle entity in place (the prefab-propagation path). It
    // must keep its slot, not sort to the back as the freshest placement.
    let doc = w.entity_document(b).expect("live entity has a document");
    assert!(w.replace_entity(doc));
    assert_eq!(w.ids_with_particles(), vec![a, b, c]);
}

#[test]
fn spawn_after_despawn_sorts_to_the_back() {
    let mut w = World::new();
    let a = w.spawn("a".into());
    let b = w.spawn("b".into());
    w.set_particles(a, Some(emitter()));
    w.set_particles(b, Some(emitter()));
    w.despawn(a);
    let d = w.spawn("d".into());
    w.set_particles(d, Some(emitter()));
    assert_eq!(w.ids_with_particles(), vec![b, d]);
}

#[test]
fn matches_the_order_filter_reference_on_a_mixed_scene() {
    // Reference implementation: walk insertion order, keep carriers. The
    // narrow query must agree with it exactly on a scene that mixes carriers,
    // non-carriers, despawns, and in-place rebuilds.
    let mut w = World::new();
    let ids: Vec<u32> = (0..10).map(|i| w.spawn(format!("e{i}"))).collect();
    for &id in ids.iter().step_by(3) {
        w.set_particles(id, Some(emitter()));
    }
    w.despawn(ids[3]);
    let doc = w.entity_document(ids[6]).expect("live entity");
    assert!(w.replace_entity(doc));
    let reference: Vec<u32> = w
        .ids()
        .iter()
        .copied()
        .filter(|&id| w.has_particles(id))
        .collect();
    assert_eq!(w.ids_with_particles(), reference);
}
