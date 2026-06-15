//! src/components/particle.rs — ParticleEmitter component + live particle state.
//!
//! A simple 2D-billboard particle system (issue #13): camera-facing textured
//! sprites with alpha + tint, per-particle lifetime, gravity, fade/grow over life,
//! continuous/burst emission, alpha/additive blending, a max-particle cap, and
//! point/sphere-vs-collider collision (die or bounce).
//!
//! The component splits cleanly into **config** (everything serde-persists, the
//! authoring knobs) and **runtime** (the live `Vec<Particle>` + spawn accumulator +
//! PRNG, all `#[serde(skip)]` — transient play-mode state). The simulation in
//! `app::particles` owns the runtime; the draw pass in `render::particles` only
//! reads it. The sim knows nothing about rendering.
//!
//! Determinism: emission/velocity jitter comes from a seeded xorshift PRNG carried
//! in the runtime and reset from `seed` on (re)entering Play, so a headless replay
//! is bit-for-bit identical. No wall-clock / unseeded RNG (determinism guard).

use glam::Vec3;
use serde::{Deserialize, Serialize};

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
    /// Size at birth (world units); the rendered size lerps start->end over life.
    pub size_start: f32,
    pub size_end: f32,
    /// Base RGBA tint at birth; alpha (and colour) fade over life.
    pub color: [f32; 4],
}

impl Particle {
    /// Normalised life in `[0, 1]` (0 at birth, 1 at death).
    pub fn life_t(&self) -> f32 {
        if self.lifetime <= 0.0 {
            1.0
        } else {
            (self.age / self.lifetime).clamp(0.0, 1.0)
        }
    }

    /// Current world size, lerped from `size_start` to `size_end` over life.
    pub fn current_size(&self) -> f32 {
        let t = self.life_t();
        self.size_start + (self.size_end - self.size_start) * t
    }

    /// Current RGBA: base colour with alpha faded linearly to zero over life.
    pub fn current_color(&self) -> [f32; 4] {
        let fade = 1.0 - self.life_t();
        [
            self.color[0],
            self.color[1],
            self.color[2],
            self.color[3] * fade,
        ]
    }
}

/// Tiny seeded xorshift64* PRNG. Deterministic, self-contained — no `rand` crate
/// (banned in sim modules by the determinism guard). Seeded per-emitter so two
/// runs with the same seed produce identical particle streams.
#[derive(Clone, Copy, Debug)]
pub struct ParticleRng {
    state: u64,
}

impl ParticleRng {
    pub fn new(seed: u64) -> Self {
        // Avoid the all-zero fixed point of xorshift.
        Self {
            state: seed ^ 0x9E37_79B9_7F4A_7C15,
        }
    }

    /// Next raw u64 (xorshift64*).
    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform f32 in `[0, 1)`.
    pub fn unit(&mut self) -> f32 {
        // Top 24 bits -> [0,1) keeps it exact in f32.
        (self.next_u64() >> 40) as f32 / (1u32 << 24) as f32
    }

    /// Uniform f32 in `[-1, 1)`.
    pub fn signed(&mut self) -> f32 {
        self.unit() * 2.0 - 1.0
    }
}

/// Transient per-emitter runtime: live particles, the fractional spawn carry, the
/// PRNG, and whether a one-shot burst has fired. Reset on entering Play.
#[derive(Clone, Debug)]
pub struct EmitterRuntime {
    pub particles: Vec<Particle>,
    /// Accumulated fractional particles owed by continuous emission.
    pub spawn_accum: f32,
    pub rng: ParticleRng,
    /// For a non-looping `Burst`: set once the single burst has been emitted.
    pub burst_fired: bool,
    /// Whether the runtime has been seeded for the current Play session.
    pub initialized: bool,
}

impl Default for EmitterRuntime {
    fn default() -> Self {
        Self {
            particles: Vec::new(),
            spawn_accum: 0.0,
            rng: ParticleRng::new(0),
            burst_fired: false,
            initialized: false,
        }
    }
}

/// Authoring component: a particle emitter attached to an entity. Emits at the
/// entity's transform position. All fields below serde-persist.
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
    /// Burst size (particles released at once in `Burst` mode).
    pub burst_count: u32,
    /// Hard cap on live particles (perf guard); spawns past it are dropped.
    pub max_particles: u32,
    /// `true` re-emits forever; `false` is one-shot (burst fires once / continuous
    /// runs until the emitter is disabled).
    pub looping: bool,

    /// Particle lifetime in seconds.
    pub lifetime: f32,
    /// Initial speed magnitude (units/second) along `direction` + cone spread.
    pub speed: f32,
    /// Base emission direction (world space; normalised at spawn).
    pub direction: Vec3,
    /// Cone half-spread in `[0, 1]` randomising the direction (0 = straight).
    pub spread: f32,
    /// Constant acceleration (e.g. `(0, -9.81, 0)` for gravity).
    pub gravity: Vec3,

    /// Sprite size at birth / death (world units), lerped over life.
    pub size_start: f32,
    pub size_end: f32,
    /// Base RGBA tint; alpha fades to zero over each particle's life.
    pub color: [f32; 4],

    /// Restitution for `Bounce` (0 = stop, 1 = perfectly elastic).
    pub bounciness: f32,
    /// Seed for the deterministic PRNG (reproducible headless replays).
    pub seed: u64,

    /// Live runtime — transient, never serialized.
    #[serde(skip)]
    pub runtime: EmitterRuntime,
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
            lifetime: 1.5,
            speed: 2.0,
            direction: Vec3::Y,
            spread: 0.2,
            gravity: Vec3::ZERO,
            size_start: 0.25,
            size_end: 0.25,
            color: [1.0, 1.0, 1.0, 1.0],
            bounciness: 0.5,
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
}
