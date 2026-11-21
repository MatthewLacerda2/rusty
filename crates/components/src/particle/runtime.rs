use glam::{Quat, Vec3};

use super::simulation::GpuSim;
use super::space::EmitterFrame;
use rusty_core::random::Random;

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
    pub size: f32,
    /// Sprite rotation in radians (about the view axis).
    pub rotation: f32,
    pub color: [f32; 4],
    pub start_frame: u32,
    pub axis: Vec3,
    pub turn: Quat,
    /// Four numbers a script keeps per particle (Unity's *Custom Data*): zero at
    /// birth, never read by the engine, written by `Particles.SetParticles`.
    pub custom: [f32; 4],
}

impl Particle {
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
            turn: Quat::IDENTITY,
            custom: [0.0; 4],
        }
    }

    pub fn orientation(&self) -> Quat {
        let axis = self.axis.try_normalize().unwrap_or(Vec3::Y);
        let tumble = Quat::from_axis_angle(axis, self.rotation);
        if self.turn == Quat::IDENTITY {
            tumble
        } else {
            self.turn * tumble
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
    pub gpu: GpuSim,
    pub spawn_accum: f32,
    pub rng: Random,
    pub burst_fired: bool,
    pub initialized: bool,
    /// Sub-emitter firings not yet dispatched.
    pub pending: Vec<SubEmitEvent>,
    pub anchor: Option<EmitterFrame>,
    pub base: Option<EmitterFrame>,
    pub revision: u64,
}

impl Default for EmitterRuntime {
    fn default() -> Self {
        Self {
            particles: Vec::new(),
            gpu: GpuSim::default(),
            spawn_accum: 0.0,
            rng: Random::new(0),
            burst_fired: false,
            initialized: false,
            pending: Vec::new(),
            anchor: None,
            base: None,
            revision: 0,
        }
    }
}

impl super::ParticleEmitterComponent {
    pub fn config(&self) -> Self {
        macro_rules! copy {
            ($($field:ident),* $(,)?) => {{
                let Self { runtime: _, $($field),* } = self;
                Self { runtime: EmitterRuntime::default(), $($field: Clone::clone($field)),* }
            }};
        }
        copy!(
            active,
            emit_mode,
            blend,
            collision,
            texture,
            rate,
            burst_count,
            max_particles,
            looping,
            simulation_space,
            simulation,
            shape,
            emit_from,
            lifetime,
            speed,
            direction,
            spread,
            gravity,
            size,
            rotation,
            color,
            size_over_life,
            color_over_life,
            drag,
            rotation_speed,
            bounciness,
            sub_emitters,
            seed,
            render,
        )
    }
}
