//! crates/render/src/passes/particles/gpu/record.rs — a GPU-simulated emitter's
//! records, the Rust twins of `engine/shaders/particles/gpu.wgsl`'s, and what they
//! are made from: a birth's record, the over-life lookup table, the emitter's
//! uniform. Their CPU twins of the step and the sprite (`twin`) are what the tests
//! hold the shaders to. Pure CPU, so it is unit-tested without an adapter.

use glam::Mat4;

use crate::passes::particles::instance::mode_code;
use rusty_components::particle::{Birth, ParticleEmitterComponent};

/// A slot's `next` when nothing lives in it (`EMPTY` in the shader): zero, so an
/// all-zero buffer (a new one, or one `clear_buffer` emptied) holds no particle.
pub(crate) const EMPTY: u32 = 0;
/// Samples per over-life curve (`LUT_SIZE` in the shader).
pub(crate) const LUT_SIZE: usize = 64;

/// One particle on the GPU (`GpuParticle`).
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct GpuParticle {
    pub(crate) position: [f32; 3],
    pub(crate) lifetime: f32,
    pub(crate) velocity: [f32; 3],
    pub(crate) age: f32,
    pub(crate) color: [f32; 4],
    pub(crate) size: f32,
    pub(crate) rotation: f32,
    pub(crate) start_frame: u32,
    /// The first step it has yet to take (steps count from 1), or [`EMPTY`].
    pub(crate) next: u32,
}

impl GpuParticle {
    /// An empty slot.
    pub(crate) const EMPTY: Self = Self {
        position: [0.0; 3],
        lifetime: 0.0,
        velocity: [0.0; 3],
        age: 0.0,
        color: [0.0; 4],
        size: 0.0,
        rotation: 0.0,
        start_frame: 0,
        next: EMPTY,
    };

    /// `birth` as it starts on the GPU: at the step it was born, or an empty slot
    /// when it died before it was uploaded.
    pub(crate) fn born(birth: &Birth) -> Self {
        if birth.dead {
            return Self::EMPTY;
        }
        let p = &birth.particle;
        Self {
            position: p.position.to_array(),
            lifetime: p.lifetime,
            velocity: p.velocity.to_array(),
            age: p.age,
            color: p.color,
            size: p.size,
            rotation: p.rotation,
            start_frame: p.start_frame,
            next: birth.step as u32 + 1,
        }
    }
}

/// One birth upload (`GpuBirth`): the particle, its slot, and the time its steps
/// older than the uploaded window took (0 when it has none).
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct GpuBirth {
    pub(crate) particle: GpuParticle,
    pub(crate) slot: u32,
    pub(crate) jumped: f32,
    pub(crate) pad: [u32; 2],
}

/// One emitter's uniform (`GpuEmitter`).
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct GpuEmitter {
    pub(crate) frame: [f32; 16],
    /// Gravity in the stored space; the time the steps older than the window
    /// took for a particle stepped at the last upload (0 when it has none).
    pub(crate) gravity: [f32; 4],
    pub(crate) light: [f32; 4],
    pub(crate) sheet: [f32; 4],
    pub(crate) stretch: [f32; 4],
    /// Mode code, slots in use, the step to move to, the step of `dts[0]`.
    pub(crate) info: [u32; 4],
    /// Births this upload, `dts` in use.
    pub(crate) counts: [u32; 4],
}

impl GpuEmitter {
    /// `emitter`'s look and motion under `frame` (its frame for a `Local` emitter,
    /// else identity), lit by `light`; the counts are the caller's.
    pub(crate) fn new(emitter: &ParticleEmitterComponent, frame: Mat4, light: [f32; 4]) -> Self {
        let render = &emitter.render;
        let book = &render.flipbook;
        // Gravity is a world acceleration: a `Local` emitter's particles are stored
        // in its own space, so it turns (and scales) into that space.
        let gravity = frame
            .inverse()
            .transform_vector3(emitter.gravity)
            .to_array();
        let gravity = if gravity.iter().all(|g| g.is_finite()) {
            gravity
        } else {
            emitter.gravity.to_array()
        };
        Self {
            frame: frame.to_cols_array(),
            gravity: [gravity[0], gravity[1], gravity[2], 0.0],
            light,
            sheet: [
                book.columns.max(1) as f32,
                book.rows.max(1) as f32,
                book.cycles.max(0.0),
                render.soft_distance,
            ],
            stretch: [
                render.length_scale,
                render.speed_scale,
                render.reactive.gpu(),
                0.0,
            ],
            info: [mode_code(render.mode), 0, 0, 0],
            counts: [0; 4],
        }
    }
}

/// `emitter`'s over-life curves sampled `LUT_SIZE` times over life: row 0 the size
/// multiplier, drag and spin (degrees/second), row 1 the colour gradient.
pub(crate) fn bake_lut(emitter: &ParticleEmitterComponent) -> Vec<[f32; 4]> {
    let at = |i: usize| i as f32 / (LUT_SIZE - 1) as f32;
    let curves = (0..LUT_SIZE).map(|i| {
        let t = at(i);
        [
            emitter.size_over_life.evaluate(t),
            emitter.drag.evaluate(t),
            emitter.rotation_speed.evaluate(t),
            0.0,
        ]
    });
    let colors = (0..LUT_SIZE).map(|i| emitter.color_over_life.evaluate(at(i)));
    curves.chain(colors).collect()
}

#[cfg(test)]
#[path = "record_tests.rs"]
mod tests;
