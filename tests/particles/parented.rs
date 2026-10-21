//! An emitter on a child spawns at the child's **world** position (#1196), the
//! one `render::passes::particles::draw` reads: from the play loop's tick and from
//! `Particles.Emit` / `Particles.Burst` alike.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use mlua::Lua;
use rusty::components::particle::{EmitMode, ParticleEmitterComponent};
use rusty::core::curve::Range;
use rusty::scene::Scene;
use rusty::scripting::ConsoleLogs;

const PIVOT: Vec3 = Vec3::new(50.0, 2.0, -3.0);

/// A still emitter (no speed, no gravity) at local origin under a pivot at `PIVOT`.
fn parented(mode: EmitMode) -> (Scene, u32) {
    let mut scene = Scene::new();
    let pivot = scene.add_entity("Pivot".to_string());
    scene.world.transform_mut(pivot).unwrap().position = PIVOT;
    let id = scene.add_entity("Emitter".to_string());
    scene.set_parent(id, Some(pivot)).unwrap();
    let emitter = ParticleEmitterComponent {
        emit_mode: mode,
        rate: 60.0,
        speed: Range::constant(0.0),
        gravity: Vec3::ZERO,
        ..Default::default()
    };
    scene.world.set_particles(id, Some(emitter));
    (scene, id)
}

fn all_at_pivot(scene: &Scene, id: u32) {
    let particles = &scene.world.particles(id).unwrap().runtime.particles;
    assert!(!particles.is_empty(), "something spawned");
    for p in particles {
        assert!(p.position.abs_diff_eq(PIVOT, 1e-3), "{}", p.position);
    }
}

#[test]
fn the_tick_spawns_at_the_world_position() {
    let (scene, id) = parented(EmitMode::Continuous);
    let console = Rc::new(RefCell::new(ConsoleLogs::new()));
    let mut world = crate::play::world(Rc::new(RefCell::new(scene)), 20.0, console);
    world.set_playing(true);
    for _ in 0..5 {
        world.tick(1.0 / 60.0);
    }
    all_at_pivot(&world.scene().borrow(), id);
}

#[test]
fn emit_and_burst_spawn_at_the_world_position() {
    let (scene, id) = parented(EmitMode::Burst);
    let scene = RefCell::new(scene);
    let lua = Lua::new();
    lua.scope(|scope| {
        rusty::api::particle::register(&lua, scope, &scene).unwrap();
        lua.load(format!("Particles.Emit({id}, 3); Particles.Burst({id})"))
            .exec()
            .unwrap();
        Ok(())
    })
    .unwrap();
    all_at_pivot(&scene.borrow(), id);
}
