//! The pair across a tick, in the play loop: a table read in `Update` and
//! written in `LateUpdate` is stale (the tick moved every age on), and an
//! inactive `Local` emitter written by another script keeps every particle under
//! one frame.

use std::cell::RefCell;
use std::rc::Rc;

use glam::{Quat, Vec3};
use rusty::components::particle::{Flipbook, ParticleRenderMode, SimulationSpace};
use rusty::core::curve::Range;
use rusty::scene::Scene;

use super::play::{burst, particles, world};
use crate::play::{attach_script, playing, run};

#[test]
fn a_table_read_before_the_tick_takes_the_hidden_defaults() {
    const ACROSS: &str = "local held
    return { Update = function(id) held = Particles.GetParticles(id) end,
      LateUpdate = function(id) if #held > 0 then Particles.SetParticles(id, held) end end }";
    let mut cfg = burst(3, 100.0);
    cfg.render.mode = ParticleRenderMode::Mesh;
    cfg.render.flipbook = Flipbook {
        columns: 4,
        rows: 4,
        random_start: true,
        ..Default::default()
    };
    let (mut world, id) = world(cfg, ACROSS, "rusty_2171_across.lua");
    run(&mut world, 1);
    assert!(
        particles(&world, id).iter().any(|p| p.axis != Vec3::Y),
        "spawned axes vary"
    );
    run(&mut world, 1);
    for p in particles(&world, id) {
        assert_eq!(
            (p.axis, p.start_frame, p.turn),
            (Vec3::Y, 0, Quat::IDENTITY)
        );
    }
}

#[test]
fn an_inactive_local_emitter_keeps_its_particles_under_one_frame() {
    let mut scene = Scene::new();
    let id = scene.add_entity("Sparks".to_string());
    let mut cfg = burst(2, 100.0);
    (cfg.simulation_space, cfg.speed) = (SimulationSpace::Local, Range::constant(0.0));
    scene.world.set_particles(id, Some(cfg));
    let driver = scene.add_entity("Driver".to_string());
    // Adds one particle 1 above the emitter, once, while the emitter is inactive.
    let add = format!(
        "local done = false
        return {{ Update = function()
          if done or Scene.IsActive({id}) then return end
          done = true
          local ps = Particles.GetParticles({id})
          local x, y, z = Transform.GetPosition({id})
          ps[#ps + 1] = {{ position = {{ x = x, y = y + 1, z = z }} }}
          Particles.SetParticles({id}, ps) end }}"
    );
    attach_script(&mut scene, driver, "rusty_2171_inactive.lua", &add);
    let mut world = playing(Rc::new(RefCell::new(scene)), 20.0);
    run(&mut world, 1);
    let step = |world: &mut rusty::app::GameWorld, active: bool, to: Vec3| {
        let mut s = world.scene().borrow_mut();
        s.world.set_active(id, active);
        s.world.set_world_position(id, to);
    };
    step(&mut world, false, Vec3::new(5.0, 0.0, 0.0));
    run(&mut world, 1);
    step(&mut world, true, Vec3::new(5.0, 0.0, 3.0));
    run(&mut world, 1);
    let at = world.scene().borrow().world.world_position(id);
    let live = particles(&world, id);
    assert_eq!(live.len(), 3, "the driver added one");
    for p in &live[..2] {
        assert!(p.position.abs_diff_eq(at, 1e-5), "{p:?} at {at}");
    }
    assert!(
        live[2].position.abs_diff_eq(at + Vec3::Y, 1e-5),
        "{:?} at {at}",
        live[2]
    );
}
