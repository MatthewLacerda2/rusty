//! src/components/particle/ — ParticleEmitter component + live particle state.
//!
//! A 2D-billboard particle system (issue #13) grown into the emitter modules
//! combat VFX need (#439): emission **shapes** ([`EmitShape`]), random **start
//! ranges** (lifetime, speed, size, rotation, colour — [`Range`]), **over-life**
//! curves and a gradient (size, colour/alpha, drag, rotation speed — [`Curve`],
//! [`Gradient`]), and **sub-emitters** that fire another emitter's burst at a
//! particle's birth, death or collision ([`SubEmitters`]).
//!
//! The component splits cleanly into **config** (everything serde-persists, the
//! authoring knobs) and **runtime** (the live `Vec<Particle>`, spawn accumulator,
//! PRNG and pending sub-emitter events, all `#[serde(skip)]`). The simulation in
//! `app::particles` owns the runtime; the draw pass in `render::passes::particles`
//! only reads it.
//!
//! Determinism: every random draw comes from a per-emitter [`Random`] stream (the
//! engine's seeded generator, `core::random`, #443) restarted from `seed` on
//! entering Play, so a headless replay is bit-for-bit identical. Per emitter, not
//! the world's shared resource, so a script drawing a number never changes how an
//! unrelated explosion looks.
//!
//! [`Random`]: crate::core::random::Random

mod legacy;
mod runtime;
mod shape;
mod spawn;
mod sub_emitter;

pub use legacy::LegacyEmitter;
pub use runtime::{EmitterRuntime, Particle, SubEmitEvent};
pub use shape::{EmitFrom, EmitShape};
pub use sub_emitter::{SubEmitTrigger, SubEmitters};

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::core::curve::{ColorRange, Curve, Gradient, Range};

/// How particles are released over time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmitMode {
    /// Steady stream at `rate` particles/second (fire, smoke).
    Continuous,
    /// All `burst_count` particles at once, then (if not looping) stop (explosions).
    Burst,
}

/// GPU blend used when drawing the sprites.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParticleBlend {
    /// Standard src-alpha blending (smoke).
    Alpha,
    /// Additive — colours sum, brightens the HDR target (sparks, fire).
    Additive,
}

/// What a particle does when its motion segment hits a collider.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CollisionResponse {
    /// Ignore colliders entirely (cheapest).
    None,
    /// Despawn on first contact (bullet impacts, splatter).
    Die,
    /// Reflect velocity, scaled by `bounciness` (restitution).
    Bounce,
}

/// Authoring component: a particle emitter attached to an entity. Emits at the
/// entity's transform position. All fields below serde-persist; every field added
/// by #439 has a default that reproduces the pre-#439 look, so old scenes load.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParticleEmitterComponent {
    pub active: bool,
    pub emit_mode: EmitMode,
    pub blend: ParticleBlend,
    pub collision: CollisionResponse,
    /// Optional sprite texture path; falls back to the renderer's default texture.
    #[serde(default)]
    pub texture: Option<String>,

    /// Continuous emission rate (particles/second).
    pub rate: f32,
    /// Burst size (particles released at once in `Burst` mode, and per sub-emit).
    pub burst_count: u32,
    /// Hard cap on live particles (perf guard); spawns past it are dropped.
    pub max_particles: u32,
    /// `true` re-emits forever; `false` is one-shot (burst fires once / continuous
    /// runs until the emitter is disabled).
    pub looping: bool,

    /// Where particles are born and which way they launch, around `direction`.
    #[serde(default)]
    pub shape: EmitShape,
    /// Emit from the shape's volume or only its surface / edge.
    #[serde(default)]
    pub emit_from: EmitFrom,
    /// Particle lifetime in seconds (random per particle within the range).
    pub lifetime: Range,
    /// Initial speed (units/second) along the shape's launch direction.
    pub speed: Range,
    /// Shape axis / base emission direction (world space; normalised at spawn).
    pub direction: Vec3,
    /// Extra random jitter on the launch direction in `[0, 1]` (0 = none).
    pub spread: f32,
    /// Constant acceleration (e.g. `(0, -9.81, 0)` for gravity).
    pub gravity: Vec3,

    /// Size at birth (world units). Loads the legacy `size_start` key.
    #[serde(alias = "size_start")]
    pub size: Range,
    /// Starting sprite rotation in degrees.
    #[serde(default = "zero_range")]
    pub rotation: Range,
    /// Base RGBA tint at birth, picked between `min` and `max`.
    pub color: ColorRange,

    /// Multiplier on the start size over each particle's life.
    #[serde(default)]
    pub size_over_life: Curve,
    /// Multiplier on the start colour over life (alpha keys fade it).
    #[serde(default = "Gradient::fade_out")]
    pub color_over_life: Gradient,
    /// Linear drag over life: the fraction of velocity lost per second.
    #[serde(default = "zero_curve")]
    pub drag: Curve,
    /// Sprite spin over life, degrees/second.
    #[serde(default = "zero_curve")]
    pub rotation_speed: Curve,

    /// Restitution for `Bounce` (0 = stop, 1 = perfectly elastic).
    pub bounciness: f32,
    /// Emitters fired at a particle's birth / death / collision.
    #[serde(default)]
    pub sub_emitters: SubEmitters,
    /// Seed for the deterministic PRNG (reproducible headless replays).
    pub seed: u64,

    /// Live runtime — transient, never serialized.
    #[serde(skip)]
    pub runtime: EmitterRuntime,
}

fn zero_range() -> Range {
    Range::constant(0.0)
}

fn zero_curve() -> Curve {
    Curve::constant(0.0)
}

impl Default for ParticleEmitterComponent {
    fn default() -> Self {
        Self {
            active: true,
            emit_mode: EmitMode::Continuous,
            blend: ParticleBlend::Alpha,
            collision: CollisionResponse::None,
            texture: None,
            rate: 20.0,
            burst_count: 32,
            max_particles: 256,
            looping: true,
            shape: EmitShape::Point,
            emit_from: EmitFrom::Volume,
            lifetime: Range::constant(1.5),
            speed: Range::constant(2.0),
            direction: Vec3::Y,
            spread: 0.2,
            gravity: Vec3::ZERO,
            size: Range::constant(0.25),
            rotation: zero_range(),
            color: ColorRange::constant([1.0; 4]),
            size_over_life: Curve::default(),
            color_over_life: Gradient::fade_out(),
            drag: zero_curve(),
            rotation_speed: zero_curve(),
            bounciness: 0.5,
            sub_emitters: SubEmitters::default(),
            seed: 1,
            runtime: EmitterRuntime::default(),
        }
    }
}

impl ParticleEmitterComponent {
    /// Number of currently live particles.
    pub fn live_count(&self) -> usize {
        self.runtime.particles.len()
    }

    /// A live particle's current world size: start size × the size curve.
    pub fn size_of(&self, p: &Particle) -> f32 {
        p.size * self.size_over_life.evaluate(p.life_t())
    }

    /// A live particle's current RGBA: start colour × the colour gradient.
    pub fn color_of(&self, p: &Particle) -> [f32; 4] {
        let g = self.color_over_life.evaluate(p.life_t());
        std::array::from_fn(|i| p.color[i] * g[i])
    }

    /// Rewrite the sub-emitter entity references through `map` (prefab save /
    /// stamp, #420/#449); a reference `map` drops is cleared.
    pub fn remap_refs(&mut self, map: &dyn Fn(u32) -> Option<u32>) {
        self.sub_emitters.remap_refs(map);
    }
}
