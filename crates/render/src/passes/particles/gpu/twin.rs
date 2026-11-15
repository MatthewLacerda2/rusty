//! crates/render/src/passes/particles/gpu/twin.rs — the CPU twins of a
//! GPU-simulated emitter's shaders (#2170): `cs_step` ([`advance`], with its
//! closed-form [`jump`]) and `vs_gpu` ([`sprite`]), step for step. The renderer
//! never runs them (it always runs the compute pass: its other passes already need
//! compute and vertex-stage storage); they are what the tests hold the shaders to.

use glam::{Mat4, Vec3, Vec4};

use super::record::{GpuEmitter, GpuParticle, EMPTY, LUT_SIZE};
use crate::passes::particles::instance::ParticleInstance;

impl GpuParticle {
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
        self.next != EMPTY
    }
}

/// The lookup table at `t` for `row`, linear between samples (`lut_at`).
pub(crate) fn lut_at(lut: &[[f32; 4]], row: usize, t: f32) -> Vec4 {
    let x = t.clamp(0.0, 1.0) * (LUT_SIZE - 1) as f32;
    let i = (x.floor() as usize).min(LUT_SIZE - 2);
    let base = row * LUT_SIZE;
    Vec4::from(lut[base + i]).lerp(Vec4::from(lut[base + i + 1]), x - i as f32)
}

/// Move `p` up to step `goal` by the window `dts`, whose first is step `first`'s
/// `dt`: the shader's `cs_step`, step for step. The steps older than the window
/// are one closed-form [`jump`] at the window's oldest `dt`; the window's own are
/// taken one at a time, so the loop never runs more than `dts.len()` times.
pub(crate) fn advance(
    p: &mut GpuParticle,
    (goal, first): (u32, u32),
    gravity: Vec3,
    lut: &[[f32; 4]],
    dts: &[f32],
) {
    if !p.alive() || p.next > goal || dts.is_empty() {
        return;
    }
    if p.next < first {
        jump(p, first - p.next, gravity, lut, dts[0]);
        p.next = if is_dead(p) { EMPTY } else { first };
        if !p.alive() {
            return;
        }
    }
    let (mut position, mut velocity) = (Vec3::from(p.position), Vec3::from(p.velocity));
    while p.next <= goal {
        let dt = dts[((p.next - first) as usize).min(dts.len() - 1)];
        let curves = lut_at(lut, 0, p.life_t());
        velocity += gravity * dt;
        velocity *= (1.0 - curves.y * dt).max(0.0);
        p.rotation += curves.z.to_radians() * dt;
        position += velocity * dt;
        p.age += dt;
        p.next += 1;
        if is_dead(p) {
            p.next = EMPTY;
            break;
        }
    }
    p.position = position.to_array();
    p.velocity = velocity.to_array();
}

/// `age < lifetime` failing, a NaN lifetime included, as on the CPU.
fn is_dead(p: &GpuParticle) -> bool {
    p.age.partial_cmp(&p.lifetime) != Some(std::cmp::Ordering::Less)
}

/// `n` steps of `dt` at once, the curves held where `p` is in its life: the
/// sum of the step's geometric velocity series (`v ← (v + g·dt)·k`, `k` the drag
/// factor) and of the positions it passes (the shader's `jump`).
pub(crate) fn jump(p: &mut GpuParticle, n: u32, gravity: Vec3, lut: &[[f32; 4]], dt: f32) {
    let curves = lut_at(lut, 0, p.life_t());
    let nf = n as f32;
    let k = (1.0 - curves.y * dt).max(0.0);
    let (v, x) = (Vec3::from(p.velocity), Vec3::from(p.position));
    let (v, x) = if (1.0 - k).abs() < 1e-6 {
        let v_n = v + gravity * dt * nf;
        (
            v_n,
            x + (v * nf + gravity * dt * nf * (nf + 1.0) * 0.5) * dt,
        )
    } else {
        let rest = gravity * dt * k / (1.0 - k);
        let kn = if k > 0.0 { k.powf(nf) } else { 0.0 };
        let v_n = (v - rest) * kn + rest;
        (
            v_n,
            x + ((v - rest) * k * (1.0 - kn) / (1.0 - k) + rest * nf) * dt,
        )
    };
    p.velocity = v.to_array();
    p.position = x.to_array();
    p.rotation += curves.z.to_radians() * dt * nf;
    p.age += dt * nf;
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
