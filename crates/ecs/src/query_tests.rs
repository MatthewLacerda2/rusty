use crate::World;
use rusty_components::ParticleEmitterComponent;

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

#[test]
fn walks_count_from_the_first_ask() {
    use crate::world::walks;
    let mut w = World::new();
    let id = w.spawn("e".into());
    w.set_particles(id, Some(emitter()));
    std::thread::spawn(move || {
        let _ = w.ids_with_component::<ParticleEmitterComponent>();
        let armed = walks::of::<ParticleEmitterComponent>();
        assert_eq!(armed, 0, "a walk before the first ask is not counted");
        let _ = w.ids_with_component::<ParticleEmitterComponent>();
        assert_eq!(walks::of::<ParticleEmitterComponent>(), 1);
    })
    .join()
    .expect("the walks thread");
}

#[test]
fn moving_bodies_are_the_non_static_colliders_and_controllers_with_their_layer() {
    use glam::Vec3;
    use rusty_components::{CharacterControllerComponent as Ctl, ColliderComponent as Col};
    let mut w = World::new();
    let [col, ctl, both, wall, off, bare] =
        ["c", "k", "b", "w", "o", "n"].map(|n| w.spawn(n.into()));
    let collider = Col::from_mesh_bounds(false, Vec3::ZERO, Vec3::ZERO);
    for id in [col, both, wall, off] {
        w.set_collider(id, Some(collider.clone()));
    }
    for id in [ctl, both] {
        w.set_character_controller(id, Some(Ctl::default()));
    }
    w.set_static(wall, true);
    w.set_layer(off, 7);
    let every = vec![(col, 0), (ctl, 0), (both, 0), (off, 7)];
    assert_eq!(w.moving_bodies(), every, "every layer, with it");
    assert!(w.is_moving_body(both) && !w.is_moving_body(wall) && !w.is_moving_body(bare));
}
