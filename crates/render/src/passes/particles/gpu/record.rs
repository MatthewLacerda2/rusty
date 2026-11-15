//! crates/render/src/passes/particles/gpu/record.rs — a GPU-simulated emitter's
//! records, the Rust twins of `engine/shaders/particles/gpu.wgsl`'s, and what they
//! are made from: a birth's record, the over-life lookup table, the emitter's
//! uniform. Also the step and the sprite on the CPU ([`advance`], [`sprite`]): the
//! fallback for an adapter that cannot run the compute pass, and the reference the
//! tests hold the shader to. Pure CPU, so it is unit-tested without an adapter.

use glam::{Mat4, Vec3, Vec4};

use crate::passes::particles::instance::{mode_code, ParticleInstance};
use rusty_components::particle::{Birth, ParticleEmitterComponent};

/// A slot's `done` when nothing lives in it (`DEAD` in the shader).
pub(crate) const DEAD: u32 = u32::MAX;
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
    /// The step it has been moved to, or [`DEAD`].
    pub(crate) done: u32,
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
        done: DEAD,
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
            done: birth.step as u32,
        }
    }

    /// Normalised life in `[0, 1]`, as `Particle::life_t`.
    fn life_t(&self) -> f32 {
        if self.lifetime <= 0.0 {
            1.0
        } else {
            (self.age / self.lifetime).clamp(0.0, 1.0)
        }
    }

    /// Whether the slot holds a live particle.
    pub(crate) fn alive(&self) -> bool {
        self.done != DEAD
    }
}

/// One birth upload (`GpuBirth`): the particle and its slot.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct GpuBirth {
    pub(crate) particle: GpuParticle,
    pub(crate) slot: u32,
    pub(crate) pad: [u32; 3],
}

/// One emitter's uniform (`GpuEmitter`).
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct GpuEmitter {
    pub(crate) frame: [f32; 16],
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

/// The lookup table at `t` for `row`, linear between samples (`lut_at`).
pub(crate) fn lut_at(lut: &[[f32; 4]], row: usize, t: f32) -> Vec4 {
    let x = t.clamp(0.0, 1.0) * (LUT_SIZE - 1) as f32;
    let i = (x.floor() as usize).min(LUT_SIZE - 2);
    let base = row * LUT_SIZE;
    Vec4::from(lut[base + i]).lerp(Vec4::from(lut[base + i + 1]), x - i as f32)
}

/// Move `p` one step at a time up to step `goal`, each by `dt_of(step)`: the
/// shader's `cs_step`, step for step.
pub(crate) fn advance(
    p: &mut GpuParticle,
    goal: u32,
    gravity: Vec3,
    lut: &[[f32; 4]],
    dt_of: impl Fn(u32) -> f32,
) {
    let owed = goal.wrapping_sub(p.done);
    if !p.alive() || owed == 0 || owed > 0x8000_0000 {
        return;
    }
    let (mut position, mut velocity) = (Vec3::from(p.position), Vec3::from(p.velocity));
    for _ in 0..owed {
        let step = p.done.wrapping_add(1);
        let dt = dt_of(step);
        let curves = lut_at(lut, 0, p.life_t());
        velocity += gravity * dt;
        velocity *= (1.0 - curves.y * dt).max(0.0);
        p.rotation += curves.z.to_radians() * dt;
        position += velocity * dt;
        p.age += dt;
        p.done = step;
        // `age < lifetime` failing, a NaN lifetime included, as on the CPU.
        if p.age.partial_cmp(&p.lifetime) != Some(std::cmp::Ordering::Less) {
            p.done = DEAD;
            break;
        }
    }
    p.position = position.to_array();
    p.velocity = velocity.to_array();
}

/// The sprite `p` draws as under `e` with `lut`, or `None` for an empty slot: the
/// shader's `vs_gpu`, as one instance of the CPU pass.
pub(crate) fn sprite(
    p: &GpuParticle,
    e: &GpuEmitter,
    lut: &[[f32; 4]],
) -> Option<ParticleInstance> {
    if !p.alive() {
        return None;
    }
    let frame = Mat4::from_cols_array(&e.frame);
    let t = p.life_t();
    let size = (p.size * lut_at(lut, 0, t).x).max(0.0);
    let tint = lut_at(lut, 1, t);
    let velocity = frame.transform_vector3(Vec3::from(p.velocity));
    let [columns, rows, cycles, soft] = e.sheet;
    let [length_scale, speed_scale, reactive, _] = e.stretch;
    Some(ParticleInstance {
        center: frame.transform_point3(Vec3::from(p.position)).to_array(),
        size,
        rotation: p.rotation,
        color: (Vec4::from(p.color) * tint).to_array(),
        stretch: velocity
            .normalize_or_zero()
            .extend(size * length_scale + velocity.length() * speed_scale)
            .to_array(),
        light: e.light,
        sheet: [
            columns,
            rows,
            frame_of(p, columns, rows, cycles) as f32,
            soft,
        ],
        mode: e.info[0],
        reactive,
    })
}

/// The flipbook frame `p` shows, as `Flipbook::frame_of`.
fn frame_of(p: &GpuParticle, columns: f32, rows: f32, cycles: f32) -> u32 {
    let frames = (columns * rows) as u32;
    if frames <= 1 {
        return 0;
    }
    let t = p.life_t().min(0.99999);
    let played = (t * cycles * frames as f32).floor() as u32;
    played.wrapping_add(p.start_frame) % frames
}

#[cfg(test)]
#[path = "record_tests.rs"]
mod tests;
