//! Sub-emitters fire the target's burst exactly once per birth / death /
//! collision, and their entity references follow prefab remapping.

use glam::Vec3;
use rusty::components::particle::{CollisionResponse, EmitMode, ParticleEmitterComponent};
use rusty::core::curve::Range;
use rusty::scene::{ColliderComponent, ColliderShape, Entity, Scene};

use super::{count, play, step};

/// A one-shot burst of `n` particles along +X living `lifetime` seconds.
fn parent(n: u32, lifetime: f32) -> ParticleEmitterComponent {
    ParticleEmitterComponent {
        emit_mode: EmitMode::Burst,
        burst_count: n,
        looping: false,
        lifetime: Range::constant(lifetime),
        speed: Range::constant(8.0),
        direction: Vec3::X,
        spread: 0.0,
        ..Default::default()
    }
}

/// An inactive sub-emitter entity firing `burst` long-lived particles.
fn child(scene: &mut Scene, name: &str, burst: u32) -> u32 {
    let id = scene.add_entity(name.to_string());
    let cfg = ParticleEmitterComponent {
        active: false,
        burst_count: burst,
        lifetime: Range::constant(100.0),
        speed: Range::constant(0.0),
        ..Default::default()
    };
    scene.world.set_particles(id, Some(cfg));
    id
}

#[test]
fn death_and_birth_fire_once_per_particle() {
    let mut scene = Scene::new();
    let on_death = child(&mut scene, "Smoke", 2);
    let on_birth = child(&mut scene, "Flash", 1);
    let mut cfg = parent(3, 0.5);
    cfg.sub_emitters.death = Some(on_death);
    cfg.sub_emitters.birth = Some(on_birth);
    let id = scene.add_entity("Sparks".to_string());
    scene.world.set_particles(id, Some(cfg));

    let world = play(scene, 60);
    assert_eq!(count(&world, id), 0, "the sparks have all died");
    assert_eq!(count(&world, on_death), 6, "3 deaths × burst of 2");
    assert_eq!(count(&world, on_birth), 3, "3 births × burst of 1");
    step(&world, 60);
    assert_eq!(count(&world, on_death), 6, "no death fires twice");
    let w = world.borrow();
    let scene = w.scene().borrow();
    let smoke = scene.world.particles(on_death).unwrap();
    for p in &smoke.runtime.particles {
        assert!(
            p.position.x > 3.0,
            "smoke spawns where the spark died: {}",
            p.position
        );
    }
}

#[test]
fn collision_fires_at_the_hit_point_once() {
    let mut scene = Scene::new();
    let wall = scene.add_entity("Wall".to_string());
    scene.world.set_static(wall, true);
    scene.world.transform_mut(wall).unwrap().position = Vec3::new(3.0, 0.0, 0.0);
    let collider = ColliderComponent {
        active: true,
        shape: ColliderShape::Box {
            size: Vec3::new(1.0, 4.0, 4.0),
        },
        is_trigger: false,
        material: Default::default(),
        aabb_min: Vec3::ZERO,
        aabb_max: Vec3::ZERO,
    };
    scene.world.set_collider(wall, Some(collider));
    scene.update_entity_collider(wall);
    let dust = child(&mut scene, "Dust", 1);
    let mut cfg = parent(4, 5.0);
    cfg.collision = CollisionResponse::Die;
    cfg.sub_emitters.collision = Some(dust);
    let id = scene.add_entity("Debris".to_string());
    scene.world.set_particles(id, Some(cfg));

    let world = play(scene, 120);
    assert_eq!(count(&world, id), 0, "debris dies on the wall");
    assert_eq!(count(&world, dust), 4, "one dust puff per impact");
    let w = world.borrow();
    let scene = w.scene().borrow();
    for p in &scene.world.particles(dust).unwrap().runtime.particles {
        assert!(
            (p.position.x - 2.5).abs() < 0.05,
            "puff at the wall face: {}",
            p.position
        );
    }
}

#[test]
fn sub_emitter_refs_follow_prefab_remapping() {
    let mut scene = Scene::new();
    let id = scene.add_entity("Emitter".to_string());
    let mut cfg = ParticleEmitterComponent::default();
    cfg.sub_emitters.death = Some(4);
    cfg.sub_emitters.collision = Some(9);
    scene.world.set_particles(id, Some(cfg));
    let mut entity = scene.world.entity_document(id).unwrap();
    entity.remap_refs(&|r| (r == 4).then_some(40));
    let subs = entity.particles.as_ref().unwrap().sub_emitters;
    assert_eq!((subs.death, subs.collision), (Some(40), None));
    assert!(Entity::is_ref_pointer("/particles/sub_emitters/death"));
    assert!(!Entity::is_ref_pointer("/particles/burst_count"));
}
