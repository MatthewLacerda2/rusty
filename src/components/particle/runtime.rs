//! src/components/particle/runtime.rs — the transient play-mode state of an
//! emitter: live particles, spawn carry, PRNG and pending sub-emitter events.

use glam::Vec3;

use crate::core::random::Random;

/// One live particle. Transient runtime state — never serialized.
#[derive(Clone, Copy, Debug)]
pub struct Particle {
    /// World-space position.
    pub position: Vec3,
    /// World-space velocity (units/second).
    pub velocity: Vec3,
    /// Seconds lived so far.
    pub age: f32,
    /// Total lifetime in seconds; despawns when `age >= lifetime`.
    pub lifetime: f32,
    /// Size at birth (world units); the emitter's size curve scales it over life.
    pub size: f32,
    /// Sprite rotation in radians (about the view axis).
    pub rotation: f32,
    /// RGBA tint at birth; the emitter's colour gradient scales it over life.
    pub color: [f32; 4],
    /// Flipbook frame the particle starts on (random-start sheets, #440; else 0).
    pub start_frame: u32,
    /// Unit axis a mesh particle tumbles about by `rotation` (#440; else `Y`).
    pub axis: Vec3,
}

impl Particle {
    /// A particle at `position` with the render-only extras at their defaults — for
    /// tests and tools that place particles by hand.
    pub fn at(position: Vec3, size: f32, color: [f32; 4], lifetime: f32) -> Self {
        Self {
            position,
            velocity: Vec3::ZERO,
            age: 0.0,
            lifetime,
            size,
            rotation: 0.0,
            color,
            start_frame: 0,
            axis: Vec3::Y,
        }
    }

    /// Normalised life in `[0, 1]` (0 at birth, 1 at death).
    pub fn life_t(&self) -> f32 {
        if self.lifetime <= 0.0 {
            1.0
        } else {
            (self.age / self.lifetime).clamp(0.0, 1.0)
        }
    }
}

/// A sub-emitter firing owed by this emitter: `target`'s burst at `position`,
/// each spawned particle inheriting `velocity`. Queued during the tick (or by a
/// scripted emit) and dispatched by `app::particles` once every emitter has run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SubEmitEvent {
    pub target: u32,
    pub position: Vec3,
    pub velocity: Vec3,
}

/// Transient per-emitter runtime. Reset on entering Play.
#[derive(Clone, Debug)]
pub struct EmitterRuntime {
    pub particles: Vec<Particle>,
    /// Accumulated fractional particles owed by continuous emission.
    pub spawn_accum: f32,
    pub rng: Random,
    /// For a non-looping `Burst`: set once the single burst has been emitted.
    pub burst_fired: bool,
    /// Whether the runtime has been seeded for the current Play session.
    pub initialized: bool,
    /// Sub-emitter firings not yet dispatched.
    pub pending: Vec<SubEmitEvent>,
}

impl Default for EmitterRuntime {
    fn default() -> Self {
        Self {
            particles: Vec::new(),
            spawn_accum: 0.0,
            rng: Random::new(0),
            burst_fired: false,
            initialized: false,
            pending: Vec::new(),
        }
    }
}
