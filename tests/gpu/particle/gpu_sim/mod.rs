//! GPU-simulated emitters on a real adapter: the compute step moves a
//! particle where the closed form says, a `Gpu` and a `Cpu` emitter of one config
//! and seed cover the same pixels, a `Local` one follows its emitter, and a dead
//! particle is not drawn. Shared here: a play world over the particle wall scene,
//! and the red pixels of a shot.

mod frames;
mod motion;
mod pause;
mod space;

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use rusty::app::GameWorld;
use rusty::components::particle::{EmitMode, ParticleEmitterComponent, ParticleSimulation};
use rusty::core::curve::{Gradient, Range};
use rusty::dev::capture::CaptureHost;
use rusty::scene::Scene;

use super::scene::{is_red, scene_with, RED};
use crate::gpu::fog::scene::shot;

/// One red, unfading, unlit emitter at the origin firing nothing by itself.
pub fn emitter(simulation: ParticleSimulation) -> ParticleEmitterComponent {
    ParticleEmitterComponent {
        simulation,
        emit_mode: EmitMode::Burst,
        looping: false,
        burst_count: 0,
        color: rusty::core::curve::ColorRange::constant(RED),
        color_over_life: Gradient::linear([1.0; 4], [1.0; 4]),
        size: Range::constant(0.6),
        speed: Range::constant(0.0),
        spread: 0.0,
        lifetime: Range::constant(10.0),
        ..Default::default()
    }
}

/// A playing world over the wall scene holding `e`; its emitter's id.
pub fn world(e: ParticleEmitterComponent) -> (GameWorld, u32) {
    let (mut scene, id) = scene_with(Default::default(), Vec::new());
    let texture = scene.world.particles(id).and_then(|p| p.texture.clone());
    scene
        .world
        .set_particles(id, Some(ParticleEmitterComponent { texture, ..e }));
    let mut game = crate::play::playing(Rc::new(RefCell::new(scene)), 20.0);
    crate::play::run(&mut game, 1);
    (game, id)
}

/// Emit `count` particles from `id` now, through the seeded path.
pub fn emit(game: &GameWorld, id: u32, count: u32) -> u32 {
    let mut scene = game.scene().borrow_mut();
    let frame = scene.world.emitter_frame(id);
    let spawned = scene
        .world
        .particles_mut(id)
        .map(|mut p| p.emit_at(frame, count));
    spawned.unwrap_or(0)
}

/// `id`'s live particle count.
pub fn count(game: &GameWorld, id: u32) -> usize {
    game.scene()
        .borrow()
        .world
        .particles(id)
        .unwrap()
        .live_count()
}

/// Move `id` to `at`.
pub fn place(game: &GameWorld, id: u32, at: Vec3) {
    game.scene()
        .borrow_mut()
        .world
        .transform_mut(id)
        .unwrap()
        .position = at;
}

/// The red pixels of `scene`'s 64 × 64 shot, row-major; `None` without an adapter.
pub fn red(host: &mut CaptureHost, scene: &Scene, name: &str) -> Option<Vec<bool>> {
    let img = shot(host, scene, &format!("gpu_sim_{name}"))?;
    Some(img.pixels().map(|p| is_red(p.0)).collect())
}

/// The mean (x, y) of the set pixels of a 64-wide mask.
pub fn centroid(mask: &[bool]) -> Option<(f32, f32)> {
    let set: Vec<(f32, f32)> = (0..mask.len())
        .filter(|&i| mask[i])
        .map(|i| ((i % 64) as f32, (i / 64) as f32))
        .collect();
    let n = set.len() as f32;
    (!set.is_empty()).then(|| {
        let (x, y) = set.iter().fold((0.0, 0.0), |a, p| (a.0 + p.0, a.1 + p.1));
        (x / n, y / n)
    })
}

/// The share of pixels two masks agree on.
pub fn agreement(a: &[bool], b: &[bool]) -> f32 {
    a.iter().zip(b).filter(|(x, y)| x == y).count() as f32 / a.len() as f32
}
