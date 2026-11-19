//! The particle tick edits each emitter where it sits: an emitting entity never
//! changes hecs archetype across a play tick, so it keeps its other components
//! and its place in the insertion order.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use rusty::components::particle::ParticleEmitterComponent;
use rusty::ecs::walks;
use rusty::scene::Scene;
use rusty::script::scripting::ConsoleLogs;

#[test]
fn an_emitter_keeps_its_archetype_across_ticks() {
    let mut scene = Scene::new();
    scene.add_entity("before".to_string());
    let id = scene.add_entity("emitter".to_string());
    scene.world.transform_mut(id).unwrap().position = Vec3::new(0.0, 1.0, 0.0);
    scene
        .world
        .set_collider(id, Some(rusty::edit::authoring::default_collider()));
    scene.world.set_particles(
        id,
        Some(ParticleEmitterComponent {
            rate: 120.0,
            ..Default::default()
        }),
    );
    let console = Rc::new(RefCell::new(ConsoleLogs::new()));
    let mut world = crate::play::world(Rc::new(RefCell::new(scene)), 20.0, console);
    world.set_playing(true);
    world.tick(1.0 / 60.0); // Play's first tick builds what it builds
    let seq = world.scene().borrow().world.seq(id);
    let before = walks::moves();
    for _ in 0..20 {
        world.tick(1.0 / 60.0);
    }
    assert_eq!(walks::moves(), before, "a tick moved an entity's archetype");
    let scene = world.scene().borrow();
    assert!(
        scene.world.particles(id).unwrap().live_count() > 0,
        "it emitted"
    );
    assert!(scene.world.has_collider(id), "its other components stay");
    assert_eq!(scene.world.seq(id), seq);
}
