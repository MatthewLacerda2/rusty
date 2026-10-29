//! The emitter frame (#1529): a mesh particle turns with a `Local` emitter, the
//! shape is in the emitter's space in both spaces, parents' scale is ignored, a
//! zero scale holds the particles, and a `LateUpdate` move is carried that tick.

use std::cell::RefCell;
use std::rc::Rc;

use glam::{Quat, Vec3};
use rusty::app::GameWorld;
use rusty::components::particle::{
    EmitShape, ParticleEmitterComponent, ParticleRenderMode, SimulationSpace,
};
use rusty::core::curve::Range;
use rusty::scene::Scene;

use super::{emitter, state, TURN};

const DT: f32 = 1.0 / 60.0;

/// A playing world with `cfg` on an entity `setup` may parent or move, ticked once.
fn play(cfg: ParticleEmitterComponent, setup: impl FnOnce(&mut Scene, u32)) -> (GameWorld, u32) {
    let mut scene = Scene::new();
    let id = scene.add_entity("Emitter".to_string());
    scene.world.set_particles(id, Some(cfg));
    setup(&mut scene, id);
    let mut world = crate::play::playing(Rc::new(RefCell::new(scene)), 20.0);
    world.tick(DT);
    (world, id)
}

fn set_scale(world: &GameWorld, id: u32, scale: f32) {
    let mut scene = world.scene().borrow_mut();
    scene.world.transform_mut(id).unwrap().scale = Vec3::splat(scale);
}

#[test]
fn a_mesh_particle_turns_with_a_local_emitter_on_top_of_its_tumble() {
    let mut cfg = emitter(SimulationSpace::Local, 0.0);
    cfg.render.mode = ParticleRenderMode::Mesh;
    cfg.rotation = Range::new(0.0, 180.0);
    let (mut world, id) = play(cfg, |_, _| {});
    let orient = |w: &GameWorld| -> Vec<Quat> {
        let scene = w.scene().borrow();
        let e = scene.world.particles(id).unwrap();
        e.runtime
            .particles
            .iter()
            .map(|p| p.orientation())
            .collect()
    };
    let before = orient(&world);
    world
        .scene()
        .borrow_mut()
        .world
        .transform_mut(id)
        .unwrap()
        .rotation = TURN;
    world.tick(DT);
    for (b, a) in before.iter().zip(orient(&world)) {
        // The mesh pass draws `mesh_particle_matrix(position, size, orientation)`.
        assert!(a.abs_diff_eq(TURN * *b, 1e-5), "{a} != turn * {b}");
    }
}

#[test]
fn the_shape_and_direction_are_in_the_emitter_space_in_both_spaces() {
    for space in [SimulationSpace::World, SimulationSpace::Local] {
        let mut cfg = emitter(space, 2.0);
        cfg.shape = EmitShape::Point;
        let (world, id) = play(cfg, |s, id| {
            s.world.transform_mut(id).unwrap().rotation = TURN;
        });
        for (_, v) in state(&world, id) {
            let want = TURN * Vec3::new(2.0, 0.0, 0.0);
            assert!(
                v.abs_diff_eq(want, 1e-5),
                "{space}: launched {v}, want {want}"
            );
        }
    }
}

#[test]
fn a_parent_scale_does_not_shrink_the_shape_or_speed() {
    let (world, id) = play(emitter(SimulationSpace::Local, 3.0), |s, id| {
        let rig = s.add_entity("Rig".to_string());
        s.world.transform_mut(rig).unwrap().scale = Vec3::splat(0.01);
        s.set_parent(id, Some(rig)).unwrap();
    });
    let particles = state(&world, id);
    for (_, v) in &particles {
        assert!((v.length() - 3.0).abs() < 1e-4, "speed {}", v.length());
    }
    // The unit box is not shrunk to 0.01: some offset reaches past it.
    let reach = particles.iter().map(|(p, _)| p.abs().max_element());
    assert!(reach.fold(0.0, f32::max) > 0.05);
}

#[test]
fn scaling_a_local_emitter_to_zero_and_back_puts_its_particles_back() {
    let (mut world, id) = play(emitter(SimulationSpace::Local, 0.0), |_, _| {});
    let before = state(&world, id);
    set_scale(&world, id, 0.0);
    world.tick(DT);
    set_scale(&world, id, 1.0);
    world.tick(DT);
    for ((b, _), (a, _)) in before.iter().zip(&state(&world, id)) {
        assert!(a.abs_diff_eq(*b, 1e-5), "{a} != {b}");
    }
}

/// `LateUpdate` steps the emitter 1 along X every tick.
const STEP: &str = "local x = 0\n\
return { LateUpdate = function(id)\n\
  x = x + 1\n\
  Transform.SetPosition(id, x, 0, 0)\nend }";

#[test]
fn a_late_update_move_is_carried_the_same_tick() {
    let mut cfg = emitter(SimulationSpace::Local, 0.0);
    cfg.shape = EmitShape::Point;
    let (mut world, id) = play(cfg, |s, id| {
        crate::play::attach_script(s, id, "rusty_1529_step.lua", STEP);
    });
    for _ in 0..3 {
        world.tick(DT);
        let at = world.scene().borrow().world.world_position(id);
        for (p, _) in state(&world, id) {
            assert!(
                p.abs_diff_eq(at, 1e-5),
                "particle {p} lags the emitter at {at}"
            );
        }
    }
}
