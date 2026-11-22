mod render;
mod runtime;
mod shape;
mod simulation;
mod space;
mod spawn;
mod sub_emitter;

pub use render::{Flipbook, ParticleRender, ParticleRenderMode};
pub use runtime::{EmitterRuntime, Particle, SubEmitEvent};
pub use shape::{EmitFrom, EmitShape};
pub use simulation::{Birth, GpuSim, ParticleSimulation, DT_HISTORY, GPU_MAX_PARTICLES};
pub use space::{EmitterFrame, SimulationSpace};
pub use sub_emitter::{SubEmitTrigger, SubEmitters};

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::{EntityRefs, RefVisit};
use rusty_asset::MapRef;
use rusty_core::curve::{ColorRange, Curve, Gradient, Range};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmitMode {
    Continuous,
    Burst,
}

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum ParticleBlend {
        Alpha = "Alpha",
        Additive = "Additive",
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CollisionResponse {
    None,
    Die,
    Bounce,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParticleEmitterComponent {
    pub active: bool,
    pub emit_mode: EmitMode,
    pub blend: ParticleBlend,
    pub collision: CollisionResponse,
    #[serde(default)]
    pub texture: Option<MapRef>,

    /// Continuous emission rate (particles/second).
    pub rate: f32,
    pub burst_count: u32,
    pub max_particles: u32,
    pub looping: bool,
    /// Whether live particles stay where they were born (`World`) or move, turn
    /// and scale with the emitter (`Local`).
    #[serde(default)]
    pub simulation_space: SimulationSpace,
    #[serde(default)]
    pub simulation: ParticleSimulation,

    #[serde(default)]
    pub shape: EmitShape,
    #[serde(default)]
    pub emit_from: EmitFrom,
    /// Particle lifetime in seconds (random per particle within the range).
    pub lifetime: Range,
    pub speed: Range,
    pub direction: Vec3,
    pub spread: f32,
    pub gravity: Vec3,

    /// Size at birth (world units).
    pub size: Range,
    /// Starting sprite rotation in degrees.
    #[serde(default = "zero_range")]
    pub rotation: Range,
    pub color: ColorRange,

    #[serde(default)]
    pub size_over_life: Curve,
    #[serde(default = "Gradient::fade_out")]
    pub color_over_life: Gradient,
    /// Linear drag over life: the fraction of velocity lost per second.
    #[serde(default = "zero_curve")]
    pub drag: Curve,
    /// Sprite spin over life, degrees/second.
    #[serde(default = "zero_curve")]
    pub rotation_speed: Curve,

    pub bounciness: f32,
    #[serde(default)]
    pub sub_emitters: SubEmitters,
    pub seed: u64,
    #[serde(default)]
    pub render: ParticleRender,

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
            simulation_space: SimulationSpace::World,
            simulation: ParticleSimulation::Cpu,
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
            render: ParticleRender::default(),
            runtime: EmitterRuntime::default(),
        }
    }
}

impl ParticleEmitterComponent {
    pub fn live_count(&self) -> usize {
        match self.simulation {
            ParticleSimulation::Cpu => self.runtime.particles.len(),
            ParticleSimulation::Gpu => self.runtime.gpu.live(),
        }
    }

    pub fn clear_particles(&mut self) {
        self.runtime.particles.clear();
        self.runtime.gpu.clear();
        self.runtime.revision = self.runtime.revision.wrapping_add(1);
    }

    pub fn particle_cap(&self) -> u32 {
        match self.simulation {
            ParticleSimulation::Cpu => self.max_particles,
            ParticleSimulation::Gpu => self.max_particles.min(GPU_MAX_PARTICLES),
        }
    }

    pub fn on_gpu(&self) -> bool {
        self.simulation == ParticleSimulation::Gpu
    }

    /// Never negative, as in Unity: that would mirror a mesh particle inside out.
    pub fn size_of(&self, p: &Particle) -> f32 {
        (p.size * self.size_over_life.evaluate(p.life_t())).max(0.0)
    }

    pub fn color_of(&self, p: &Particle) -> [f32; 4] {
        let g = self.color_over_life.evaluate(p.life_t());
        std::array::from_fn(|i| p.color[i] * g[i])
    }
}

/// The sub-emitters are its only references.
impl EntityRefs for ParticleEmitterComponent {
    fn visit_refs(&mut self, visit: &mut RefVisit<'_>) {
        self.sub_emitters.visit_refs(visit);
    }

    fn each_ref(&self, visit: &mut dyn FnMut(&str, u32)) {
        self.sub_emitters.each_ref(visit);
    }

    /// Test hook for other crates' id-slot tests.
    #[doc(hidden)]
    fn every_ref() -> Self {
        Self::default()
    }
}

/// Why a gradient's dropped key list is not the component's default.
const GRADIENT_LIST: &str = "a gradient written without this list reads it as `Gradient` does everywhere (no alpha keys opaque, no colour keys white), not as the component's default gradient";

impl crate::FirstClass for ParticleEmitterComponent {
    const NAME: &'static str = "Particles";
    const ALIASES: &'static [&'static str] = &["particlesystem"];
    const INVENTORY: &'static str = "ParticleEmitter";
    const DEFAULT_EXCEPTIONS: &'static [(&'static str, &'static str)] =
        &[("color_over_life.alpha", GRADIENT_LIST)];
}
