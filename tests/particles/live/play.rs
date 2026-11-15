//! The pair inside the real play loop: what `Update` writes is what the tick
//! integrates, `custom` lives with its particle, a `Local` emitter round-trips in
//! world space, and a scripted rule replays bit for bit.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use rusty::app::GameWorld;
use rusty::components::particle::{EmitShape, ParticleEmitterComponent, SimulationSpace};
use rusty::core::curve::Range;
use rusty::scene::Scene;

use crate::play::{attach_script, playing, run, DT};

/// A playing world whose one emitter `cfg` runs `script`, and the emitter's id.
pub(super) fn world(cfg: ParticleEmitterComponent, script: &str, file: &str) -> (GameWorld, u32) {
    let mut scene = Scene::new();
    let id = scene.add_entity("Sparks".to_string());
    scene.world.set_particles(id, Some(cfg));
    attach_script(&mut scene, id, file, script);
    (playing(Rc::new(RefCell::new(scene)), 20.0), id)
}

/// A one-shot point burst of `n` particles living `lifetime`, launched along +X.
pub(super) fn burst(n: u32, lifetime: f32) -> ParticleEmitterComponent {
    let mut cfg = super::emitter(n);
    cfg.shape = EmitShape::Point;
    cfg.lifetime = Range::constant(lifetime);
    cfg.speed = Range::constant(2.0);
    cfg
}

/// Every live particle of `id`.
pub(super) fn particles(world: &GameWorld, id: u32) -> Vec<rusty::components::particle::Particle> {
    let scene = world.scene().borrow();
    let live = scene.world.particles(id).unwrap().runtime.particles.clone();
    live
}

#[test]
fn a_velocity_written_in_update_is_the_one_integrated_that_tick() {
    const UP: &str = "return { Update = function(id)
      local ps = Particles.GetParticles(id)
      for _, p in ipairs(ps) do p.velocity = { x = 0, y = 10, z = 0 } end
      Particles.SetParticles(id, ps) end }";
    let (mut world, id) = world(burst(3, 100.0), UP, "rusty_2171_up.lua");
    run(&mut world, 1);
    let before = particles(&world, id);
    run(&mut world, 1);
    for (b, a) in before.iter().zip(&particles(&world, id)) {
        assert_eq!(a.velocity, Vec3::new(0.0, 10.0, 0.0));
        assert_eq!(a.position, b.position + Vec3::new(0.0, 10.0, 0.0) * DT);
    }
}

#[test]
fn a_shorter_list_kills_without_the_death_sub_emitter() {
    const KEEP_ONE: &str = "return { Update = function(id)
      local ps = Particles.GetParticles(id)
      if #ps > 1 then Particles.SetParticles(id, { ps[1] }) end end }";
    let mut scene = Scene::new();
    let smoke = scene.add_entity("Smoke".to_string());
    let child = ParticleEmitterComponent {
        active: false,
        burst_count: 2,
        ..Default::default()
    };
    scene.world.set_particles(smoke, Some(child));
    let id = scene.add_entity("Sparks".to_string());
    let mut cfg = burst(4, 100.0);
    cfg.sub_emitters.death = Some(smoke);
    scene.world.set_particles(id, Some(cfg));
    attach_script(&mut scene, id, "rusty_2171_keep.lua", KEEP_ONE);
    let mut world = playing(Rc::new(RefCell::new(scene)), 20.0);
    run(&mut world, 5);
    assert_eq!(particles(&world, id).len(), 1);
    assert!(particles(&world, smoke).is_empty(), "no death fired");
}

#[test]
fn custom_survives_ticks_and_an_older_particle_dying() {
    const TAG: &str = "local done = false
    return { Update = function(id)
      local ps = Particles.GetParticles(id)
      if done or #ps < 2 then return end
      done = true
      ps[1].lifetime = ps[1].age + 0.05
      ps[2].custom = { 7, 8, 9, 10 }
      Particles.SetParticles(id, ps) end }";
    let (mut world, id) = world(burst(2, 100.0), TAG, "rusty_2171_tag.lua");
    run(&mut world, 30);
    let live = particles(&world, id);
    assert_eq!(live.len(), 1, "the older one died");
    assert_eq!(live[0].custom, [7.0, 8.0, 9.0, 10.0]);
}

#[test]
fn a_local_emitter_moved_in_late_update_round_trips_in_world_space() {
    // Moves the emitter, then writes each particle 1 above where it now stands.
    const LIFT: &str = "local x = 0
    return { LateUpdate = function(id)
      x = x + 1
      Transform.SetPosition(id, x, 0, 0)
      local ps = Particles.GetParticles(id)
      for _, p in ipairs(ps) do p.position = { x = x, y = 1, z = 0 } end
      Particles.SetParticles(id, ps) end }";
    let mut cfg = burst(3, 100.0);
    cfg.simulation_space = SimulationSpace::Local;
    cfg.speed = Range::constant(0.0);
    let (mut world, id) = world(cfg, LIFT, "rusty_2171_lift.lua");
    for _ in 0..3 {
        run(&mut world, 1);
        let at = world.scene().borrow().world.world_position(id);
        for p in particles(&world, id) {
            assert!(p.position.abs_diff_eq(at + Vec3::Y, 1e-5), "{p:?} at {at}");
        }
    }
}

/// The doc's attractor: sparks pulled toward a point, in `Update`.
const ATTRACT: &str = "return { Update = function(id, dt)
  local ps = Particles.GetParticles(id)
  for _, p in ipairs(ps) do
    local v, x = p.velocity, p.position
    v.x = v.x + (0 - x.x) * 4 * dt
    v.y = v.y + (2 - x.y) * 4 * dt
    v.z = v.z + (0 - x.z) * 4 * dt
  end
  Particles.SetParticles(id, ps) end }";

#[test]
fn a_scripted_attractor_replays_bit_for_bit() {
    let run_once = || {
        let mut cfg = super::emitter(0);
        cfg.emit_mode = rusty::components::particle::EmitMode::Continuous;
        cfg.rate = 60.0;
        cfg.max_particles = 64;
        let (mut world, id) = world(cfg, ATTRACT, "rusty_2171_attract.lua");
        run(&mut world, 90);
        format!("{:?}", particles(&world, id))
    };
    let first = run_once();
    assert!(first.contains("Particle"), "the emitter has live particles");
    assert_eq!(first, run_once());
}
