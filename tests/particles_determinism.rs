//! Particle system determinism (issue #13).
//!
//! A seeded emitter must produce bit-for-bit identical particle state across two
//! independent runs — the contract that lets the headless harness reproduce
//! particle behaviour. Driven through the real play loop (`GameWorld::tick`).

use glam::Vec3;
use rusty::app::GameWorld;
use rusty::components::particle::{EmitMode, ParticleBlend, ParticleEmitterComponent};
use rusty::core::input::InputState;
use rusty::navigation::NavigationGraph;
use rusty::scene::Scene;
use rusty::scripting::ConsoleLogs;

/// Build a play-mode world with a single seeded emitter and run `frames` ticks,
/// returning every live particle's (position, size, alpha) as a flat vector.
fn run_emitter(seed: u64, frames: u32) -> Vec<(Vec3, f32, f32)> {
    let mut scene = Scene::new();
    let id = scene.add_entity("Emitter".to_string());
    {
        let mut e = scene.get_entity_mut(id).unwrap();
        e.transform.position = Vec3::new(0.0, 0.0, 0.0);
        e.particles = Some(ParticleEmitterComponent {
            emit_mode: EmitMode::Continuous,
            blend: ParticleBlend::Additive,
            rate: 30.0,
            lifetime: 2.0,
            speed: 3.0,
            direction: Vec3::new(0.2, 1.0, 0.1),
            spread: 0.6,
            gravity: Vec3::new(0.0, -9.81, 0.0),
            seed,
            ..Default::default()
        });
    }

    let mut world = build_world(scene);
    world.set_playing(true);
    for _ in 0..frames {
        world.tick(1.0 / 60.0);
    }

    let scene = world.scene();
    let e = scene.get_entity(id).unwrap();
    e.particles
        .as_ref()
        .unwrap()
        .runtime
        .particles
        .iter()
        .map(|p| (p.position, p.current_size(), p.current_color()[3]))
        .collect()
}

fn build_world(scene: Scene) -> GameWorld {
    let input = InputState::new();
    let nav = NavigationGraph::new(-20.0, 20.0, -20.0, 20.0, 1.0);
    let console = ConsoleLogs::new();
    GameWorld::new(scene, input, nav, console)
}

#[test]
fn seeded_emitter_is_reproducible() {
    let a = run_emitter(42, 90);
    let b = run_emitter(42, 90);
    assert!(!a.is_empty(), "emitter should have spawned particles");
    assert_eq!(
        a.len(),
        b.len(),
        "same seed must yield the same live-particle count"
    );
    for (i, ((pa, sa, ala), (pb, sb, alb))) in a.iter().zip(&b).enumerate() {
        assert_eq!(pa, pb, "particle {i} position diverged between runs");
        assert_eq!(sa, sb, "particle {i} size diverged between runs");
        assert_eq!(ala, alb, "particle {i} alpha diverged between runs");
    }
}

#[test]
fn different_seeds_diverge() {
    let a = run_emitter(1, 90);
    let b = run_emitter(2, 90);
    assert!(!a.is_empty() && !b.is_empty());
    // Overwhelmingly likely to differ; if every position matched the PRNG would be
    // ignoring its seed.
    assert_ne!(a, b, "distinct seeds should produce distinct streams");
}

#[test]
fn max_particles_cap_is_respected() {
    let mut scene = Scene::new();
    let id = scene.add_entity("Capped".to_string());
    {
        let mut e = scene.get_entity_mut(id).unwrap();
        e.particles = Some(ParticleEmitterComponent {
            emit_mode: EmitMode::Continuous,
            rate: 10_000.0,
            max_particles: 50,
            lifetime: 100.0,
            seed: 7,
            ..Default::default()
        });
    }
    let mut world = build_world(scene);
    world.set_playing(true);
    for _ in 0..120 {
        world.tick(1.0 / 60.0);
    }
    let scene = world.scene();
    let count = scene
        .get_entity(id)
        .unwrap()
        .particles
        .as_ref()
        .unwrap()
        .live_count();
    assert!(
        count <= 50,
        "live particle count {count} exceeded the cap of 50"
    );
}
