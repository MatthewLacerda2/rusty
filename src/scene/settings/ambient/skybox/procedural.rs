//! The procedural sky's light (#1675): `common::procedural_sky` evaluated on the
//! CPU and projected into L2 SH, memoised by its `SkySettings`.
//!
//! The gradient is the shader's term for term. The clouds are the same value-noise
//! FBM, carved and banded the same way, but hashed with integers instead of the
//! shader's `fract(sin(·))`, whose last bits differ by platform: the cells differ,
//! their statistics do not, and an L2 projection keeps only those (the layer's
//! brightness by height).
//!
//! The sky is affine in the ambient colour it tints from (`a·colour + b` per
//! direction, `a` one scalar), so two projections, under a white and a black
//! colour, give the SH under any colour without projecting again.

use std::sync::Mutex;

use glam::{Vec2, Vec3};

use crate::scene::lighting::probe_fill::fibonacci_sphere;
use crate::scene::{Sh9, SkySettings};

/// Directions projected: the clouds' cells average out well past the L2 bands.
const SAMPLES: usize = 4096;

/// The last settings projected and their `(a, b)` terms.
static PROJECTED: Mutex<Option<(SkySettings, Sh9, Sh9)>> = Mutex::new(None);

/// The radiance SH of the procedural sky `sky` draws under ambient colour `color`.
pub(super) fn radiance(sky: &SkySettings, color: Vec3) -> Sh9 {
    let mut kept = PROJECTED.lock().unwrap_or_else(|e| e.into_inner());
    let (a, b) = match &*kept {
        Some((at, a, b)) if at == sky => (*a, *b),
        _ => {
            let white = project(sky, Vec3::ONE);
            let b = project(sky, Vec3::ZERO);
            let mut a = white;
            for (c, z) in a.coeffs.iter_mut().zip(b.coeffs) {
                *c = (Vec3::from(*c) - Vec3::from(z)).to_array();
            }
            *kept = Some((*sky, a, b));
            (a, b)
        }
    };
    let mut out = b;
    for (o, a) in out.coeffs.iter_mut().zip(a.coeffs) {
        *o = (Vec3::from(*o) + Vec3::from(a) * color).to_array();
    }
    out
}

pub(super) fn project(sky: &SkySettings, color: Vec3) -> Sh9 {
    let mut sh = Sh9::zero();
    let weight = 4.0 * std::f32::consts::PI / SAMPLES as f32;
    for dir in fibonacci_sphere(SAMPLES) {
        sh.add_sample(dir, sky_color(sky, color, dir), weight);
    }
    sh
}

/// `common::procedural_sky` along unit `dir`, before fog.
pub(in crate::scene::settings::ambient) fn sky_color(
    sky: &SkySettings,
    ambient: Vec3,
    dir: Vec3,
) -> Vec3 {
    let [zenith, horizon, ground] = sky.resolved(ambient);
    let up = dir.y.clamp(0.0, 1.0);
    let down = (-dir.y).clamp(0.0, 1.0);
    let mut color = horizon.lerp(zenith, up.powf(0.5));
    color = color.lerp(ground, down.powf(0.35));
    if dir.y > 0.0 {
        let dome = Vec2::new(dir.x, dir.z) / dir.y.max(0.15);
        let fbm = fbm(dome * 1.6) + cloud_shift(sky.cloud_coverage);
        let coverage = smoothstep(0.45, 0.80, fbm);
        let band = smoothstep(0.05, 0.35, up);
        let cloud = zenith.lerp(Vec3::ONE, 0.85);
        color = color.lerp(cloud, coverage * band * sky.cloud_opacity);
    }
    color
}

/// `common::cloud_shift`.
fn cloud_shift(coverage: f32) -> f32 {
    let c = coverage - 0.5;
    c * if c > 0.0 { 1.6 } else { 1.1 }
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// `sky::noise::sky_fbm`: five octaves of value noise.
fn fbm(p: Vec2) -> f32 {
    let (mut sum, mut amp, mut freq) = (0.0, 0.5, p);
    for _ in 0..5 {
        sum += amp * value_noise(freq);
        freq *= 2.0;
        amp *= 0.5;
    }
    sum
}

/// `sky::noise::sky_value_noise`, smoothstep-weighted between cell corners.
fn value_noise(p: Vec2) -> f32 {
    let i = p.floor();
    let f = p - i;
    let u = f * f * (Vec2::splat(3.0) - 2.0 * f);
    let corner = |dx: f32, dy: f32| hash(i.x + dx, i.y + dy);
    let bottom = corner(0.0, 0.0) + (corner(1.0, 0.0) - corner(0.0, 0.0)) * u.x;
    let top = corner(0.0, 1.0) + (corner(1.0, 1.0) - corner(0.0, 1.0)) * u.x;
    bottom + (top - bottom) * u.y
}

/// A cell's value in `[0, 1)`, from its integer coordinates.
fn hash(x: f32, y: f32) -> f32 {
    let (x, y) = (x as i32 as u32, y as i32 as u32);
    let mut h = x.wrapping_mul(0x8da6_b343) ^ y.wrapping_mul(0xd816_3841);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h >> 8) as f32 / (1u32 << 24) as f32
}
