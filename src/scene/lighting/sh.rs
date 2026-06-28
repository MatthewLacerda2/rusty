//! src/scene/sh.rs — Spherical-harmonics (SH) math for light probes.
//!
//! Hand-rolled order-2 (L2) real spherical harmonics over `glam`, no external
//! micro-dependency. An L2 SH irradiance signal is 9 basis coefficients, each an
//! RGB triple — 27 floats total — which is exactly what one light probe stores.
//!
//! Two operations live here, both pure textbook math (deterministic, no RNG, no
//! wall-clock — this module is reachable from the sim layer):
//!   * `sh_basis(dir)`     — evaluate the 9 real SH basis functions in a direction.
//!   * `Sh9::add_sample`   — accumulate a directional radiance sample into a probe.
//!   * `Sh9::eval(normal)` — reconstruct irradiance for a surface normal, applying
//!     the standard cosine-lobe convolution (Ramamoorthi & Hanrahan 2001) so the
//!     stored radiance reads back as diffuse irradiance.
//!
//! The coefficient layout is the canonical (l, m) order:
//!   index 0:               l=0  m= 0
//!   indices 1,2,3:         l=1  m=-1,0,+1
//!   indices 4,5,6,7,8:     l=2  m=-2,-1,0,+1,+2

use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Number of L2 SH basis functions (3 bands: 1 + 3 + 5).
pub const SH_COEFFS: usize = 9;

/// Evaluate the 9 real spherical-harmonic basis functions for a unit direction.
/// Constants are the standard normalized real-SH polynomials.
pub fn sh_basis(dir: Vec3) -> [f32; SH_COEFFS] {
    let d = dir.normalize_or_zero();
    let (x, y, z) = (d.x, d.y, d.z);
    [
        0.282_094_8,                        // Y00  = 0.5*sqrt(1/pi)
        -0.488_602_5 * y,                   // Y1-1 = -sqrt(3/(4pi)) * y
        0.488_602_5 * z,                    // Y10  =  sqrt(3/(4pi)) * z
        -0.488_602_5 * x,                   // Y11  = -sqrt(3/(4pi)) * x
        1.092_548_4 * x * y,                // Y2-2 =  sqrt(15/(4pi)) * x*y
        -1.092_548_4 * y * z,               // Y2-1 = -sqrt(15/(4pi)) * y*z
        0.315_391_57 * (3.0 * z * z - 1.0), // Y20  = 0.5*sqrt(5/(4pi))*(3z^2-1)
        -1.092_548_4 * x * z,               // Y21  = -sqrt(15/(4pi)) * x*z
        0.546_274_2 * (x * x - y * y),      // Y22  = 0.25*sqrt(15/pi)*(x^2-y^2)
    ]
}

/// The diffuse cosine-lobe convolution factors per SH band (A_l), which turn a
/// stored *radiance* SH into reconstructed *irradiance*. Band 0 → pi, band 1 →
/// 2pi/3, band 2 → pi/4 (Ramamoorthi & Hanrahan). Indexed by coefficient.
const COSINE_LOBE: [f32; SH_COEFFS] = {
    let a0 = std::f32::consts::PI;
    let a1 = 2.094_395_2; // 2*pi/3
    let a2 = std::f32::consts::FRAC_PI_4; // pi/4
    [a0, a1, a1, a1, a2, a2, a2, a2, a2]
};

/// One L2 SH irradiance probe: 9 coefficients × RGB. Serialized into the lighting
/// sidecar (`<scene>.lighting.json`), never into the scene document itself.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sh9 {
    /// 9 basis coefficients, each an RGB radiance triple.
    pub coeffs: [[f32; 3]; SH_COEFFS],
}

impl Default for Sh9 {
    fn default() -> Self {
        Self {
            coeffs: [[0.0; 3]; SH_COEFFS],
        }
    }
}

impl Sh9 {
    /// The zero probe (no stored light).
    pub fn zero() -> Self {
        Self::default()
    }

    /// Accumulate one directional radiance sample `rgb` arriving from `dir` into the
    /// projection, weighted by `weight` (the sample's solid-angle share). Pure
    /// projection: `c_i += weight * rgb * Y_i(dir)`.
    pub fn add_sample(&mut self, dir: Vec3, rgb: Vec3, weight: f32) {
        let basis = sh_basis(dir);
        for (coeff, b) in self.coeffs.iter_mut().zip(basis) {
            coeff[0] += rgb.x * b * weight;
            coeff[1] += rgb.y * b * weight;
            coeff[2] += rgb.z * b * weight;
        }
    }

    /// Reconstruct diffuse irradiance for a surface normal, applying the cosine-lobe
    /// convolution. The result is clamped to non-negative (SH ringing can dip below
    /// zero). Mirrors the shader-side `eval_sh`, so the headless analytic path and the
    /// GPU agree.
    pub fn eval(&self, normal: Vec3) -> Vec3 {
        let basis = sh_basis(normal);
        let mut out = Vec3::ZERO;
        for i in 0..SH_COEFFS {
            let c = Vec3::from(self.coeffs[i]);
            out += c * basis[i] * COSINE_LOBE[i];
        }
        out.max(Vec3::ZERO)
    }

    /// Component-wise lerp between two probes (the trilinear interpolation kernel).
    pub fn lerp(a: &Sh9, b: &Sh9, t: f32) -> Sh9 {
        let mut out = Sh9::zero();
        for i in 0..SH_COEFFS {
            for c in 0..3 {
                out.coeffs[i][c] = a.coeffs[i][c] + (b.coeffs[i][c] - a.coeffs[i][c]) * t;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A constant white radiance over the sphere reconstructs to a flat, normal-
    /// independent irradiance (only the DC band survives).
    #[test]
    fn constant_radiance_is_flat() {
        let mut sh = Sh9::zero();
        let dirs = sphere_dirs();
        let w = 4.0 * std::f32::consts::PI / dirs.len() as f32;
        for d in &dirs {
            sh.add_sample(*d, Vec3::ONE, w);
        }
        let up = sh.eval(Vec3::Y);
        let side = sh.eval(Vec3::X);
        assert!(
            (up.x - side.x).abs() < 0.05,
            "flat field varied: {up} vs {side}"
        );
        assert!(up.x > 0.0);
    }

    /// A single bright direction reconstructs brighter when the normal faces it than
    /// when it faces away — the probe is directional.
    #[test]
    fn directional_is_brighter_toward_source() {
        let mut sh = Sh9::zero();
        sh.add_sample(Vec3::Y, Vec3::splat(10.0), 1.0);
        let toward = sh.eval(Vec3::Y).x;
        let away = sh.eval(Vec3::NEG_Y).x;
        assert!(toward > away, "expected {toward} > {away}");
    }

    fn sphere_dirs() -> Vec<Vec3> {
        crate::scene::lighting::probe_fill::fibonacci_sphere(512)
    }
}
