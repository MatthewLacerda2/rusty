//! The procedural sky's light: `common::procedural_sky` on the CPU, in L2 SH. Change it with the
//! shader; the clouds hash with integers, not the shader's `fract(sin(·))`, whose bits differ by
//! platform: the cells differ, their L2 statistics do not.

use std::sync::Mutex;

use glam::{Vec2, Vec3};

use crate::lighting::probe_fill::fibonacci_sphere;
use crate::{Sh9, SkySettings};

/// Directions projected: the clouds' cells average out well past the L2 bands.
const SAMPLES: usize = 4096;

// The sky is affine in the ambient colour (`a·colour + b`), so projections under white and
// black give any colour's SH: the last settings and their `(a, b)`.
static PROJECTED: Mutex<Option<(SkySettings, Sh9, Sh9)>> = Mutex::new(None);

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

pub(super) const SKY_ZENITH_EXPONENT: f32 = 0.5;
pub(super) const SKY_GROUND_EXPONENT: f32 = 0.35;
pub(super) const CLOUD_DOME_MIN_HEIGHT: f32 = 0.15;
pub(super) const CLOUD_CELLS_PER_UNIT: f32 = 1.6;
pub(super) const CLOUD_FBM_EDGE_START: f32 = 0.45;
pub(super) const CLOUD_FBM_EDGE_END: f32 = 0.80;
pub(super) const CLOUD_BAND_START: f32 = 0.05;
pub(super) const CLOUD_BAND_END: f32 = 0.35;
pub(super) const CLOUD_TO_WHITE: f32 = 0.85;

/// `common::procedural_sky` before fog (a GPU test holds the gradient to it).
pub fn sky_color(sky: &SkySettings, ambient: Vec3, dir: Vec3) -> Vec3 {
    let [zenith, horizon, ground] = sky.resolved(ambient);
    let up = dir.y.clamp(0.0, 1.0);
    let down = (-dir.y).clamp(0.0, 1.0);
    let mut color = horizon.lerp(zenith, up.powf(SKY_ZENITH_EXPONENT));
    color = color.lerp(ground, down.powf(SKY_GROUND_EXPONENT));
    if dir.y > 0.0 {
        let dome = Vec2::new(dir.x, dir.z) / dir.y.max(CLOUD_DOME_MIN_HEIGHT);
        let fbm = fbm(dome * CLOUD_CELLS_PER_UNIT) + cloud_shift(sky.cloud_coverage);
        let coverage = smoothstep(CLOUD_FBM_EDGE_START, CLOUD_FBM_EDGE_END, fbm);
        let band = smoothstep(CLOUD_BAND_START, CLOUD_BAND_END, up);
        let cloud = zenith.lerp(Vec3::ONE, CLOUD_TO_WHITE);
        color = color.lerp(cloud, coverage * band * sky.cloud_opacity);
    }
    color
}

/// Ports `sky/noise.wgsl`'s `cloud_shift`; `fbm` and `value_noise` port its `sky_fbm` and
/// `sky_value_noise`.
fn cloud_shift(coverage: f32) -> f32 {
    let c = coverage - 0.5;
    c * if c > 0.0 { 1.6 } else { 1.1 }
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn fbm(p: Vec2) -> f32 {
    let (mut sum, mut amp, mut freq) = (0.0, 0.5, p);
    for _ in 0..5 {
        sum += amp * value_noise(freq);
        freq *= 2.0;
        amp *= 0.5;
    }
    sum
}

fn value_noise(p: Vec2) -> f32 {
    let i = p.floor();
    let f = p - i;
    let u = f * f * (Vec2::splat(3.0) - 2.0 * f);
    let corner = |dx: f32, dy: f32| hash(i.x + dx, i.y + dy);
    let bottom = corner(0.0, 0.0) + (corner(1.0, 0.0) - corner(0.0, 0.0)) * u.x;
    let top = corner(0.0, 1.0) + (corner(1.0, 1.0) - corner(0.0, 1.0)) * u.x;
    bottom + (top - bottom) * u.y
}

fn hash(x: f32, y: f32) -> f32 {
    let (x, y) = (x as i32 as u32, y as i32 as u32);
    let mut h = x.wrapping_mul(0x8da6_b343) ^ y.wrapping_mul(0xd816_3841);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h >> 8) as f32 / (1u32 << 24) as f32
}
