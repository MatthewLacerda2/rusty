use crate::world::walks;
use crate::World;
use rusty_components::{MeshComponent, ParticleEmitterComponent};

fn emitter(active: bool) -> ParticleEmitterComponent {
    ParticleEmitterComponent {
        active,
        ..ParticleEmitterComponent::default()
    }
}

fn mixed_world() -> World {
    let mut w = World::new();
    let ids: Vec<u32> = (0..12).map(|i| w.spawn(format!("e{i}"))).collect();
    for (k, &id) in ids.iter().enumerate().rev() {
        w.set_particles(id, Some(emitter(k % 3 != 0)));
        if k % 2 == 0 {
            let mesh = MeshComponent::new("Asset", Vec::new(), Vec::new());
            w.set_mesh(id, Some(mesh));
        }
    }
    w.despawn(ids[1]);
    w.despawn(ids[4]);
    let late = w.spawn("late".into());
    w.set_particles(late, Some(emitter(true)));
    w
}

#[test]
fn is_the_unfiltered_walk_filtered_in_the_same_order() {
    let w = mixed_world();
    let reference: Vec<u32> = w
        .ids_with_particles()
        .into_iter()
        .filter(|&id| w.particles(id).is_some_and(|e| e.active))
        .collect();
    let kept = w.ids_with_component_where::<ParticleEmitterComponent>(|e| e.active);
    assert_eq!(kept, reference);
    assert!(!kept.is_empty() && kept.len() < w.ids_with_particles().len());
    let all = w.ids_with_component_where::<ParticleEmitterComponent>(|_| true);
    assert_eq!(all, w.ids_with_particles());
}

#[test]
fn counts_one_walk_of_the_column() {
    let w = mixed_world();
    std::thread::spawn(move || {
        let before = walks::of::<ParticleEmitterComponent>();
        let _ = w.ids_with_component_where::<ParticleEmitterComponent>(|e| e.active);
        assert_eq!(walks::of::<ParticleEmitterComponent>(), before + 1);
        let meshes = walks::of::<MeshComponent>();
        let _ = w.ids_with_mesh_where(|m| m.skin.is_none());
        assert_eq!(walks::of::<MeshComponent>(), meshes + 1);
    })
    .join()
    .expect("the walks thread");
}
