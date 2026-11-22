use glam::Vec3;
use serde::{Deserialize, Serialize};

const SH_COEFFS: usize = 9;

fn sh_basis(dir: Vec3) -> [f32; SH_COEFFS] {
    let d = dir.normalize_or_zero();
    let (x, y, z) = (d.x, d.y, d.z);
    [
        0.282_094_8,
        -0.488_602_5 * y,
        0.488_602_5 * z,
        -0.488_602_5 * x,
        1.092_548_4 * x * y,
        -1.092_548_4 * y * z,
        0.315_391_57 * (3.0 * z * z - 1.0),
        -1.092_548_4 * x * z,
        0.546_274_2 * (x * x - y * y),
    ]
}

const COSINE_LOBE: [f32; SH_COEFFS] = {
    let a0 = std::f32::consts::PI;
    let a1 = 2.094_395_2;
    let a2 = std::f32::consts::FRAC_PI_4;
    [a0, a1, a1, a1, a2, a2, a2, a2, a2]
};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sh9 {
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
    pub fn zero() -> Self {
        Self::default()
    }

    pub fn add_sample(&mut self, dir: Vec3, rgb: Vec3, weight: f32) {
        let basis = sh_basis(dir);
        for (coeff, b) in self.coeffs.iter_mut().zip(basis) {
            coeff[0] += rgb.x * b * weight;
            coeff[1] += rgb.y * b * weight;
            coeff[2] += rgb.z * b * weight;
        }
    }

    pub fn accumulate(&mut self, other: &Sh9) {
        for (a, b) in self.coeffs.iter_mut().zip(other.coeffs) {
            for c in 0..3 {
                a[c] += b[c];
            }
        }
    }

    pub fn eval(&self, normal: Vec3) -> Vec3 {
        let basis = sh_basis(normal);
        let mut out = Vec3::ZERO;
        for i in 0..SH_COEFFS {
            let c = Vec3::from(self.coeffs[i]);
            out += c * basis[i] * COSINE_LOBE[i];
        }
        out.max(Vec3::ZERO)
    }

    pub fn eval_radiance(&self, dir: Vec3) -> Vec3 {
        let basis = sh_basis(dir);
        (0..SH_COEFFS).fold(Vec3::ZERO, |out, i| {
            out + Vec3::from(self.coeffs[i]) * basis[i]
        })
    }

    pub fn convolved_over_pi(&self) -> Sh9 {
        let mut out = *self;
        for (c, a) in out.coeffs.iter_mut().zip(COSINE_LOBE) {
            *c = (Vec3::from(*c) * (a / std::f32::consts::PI)).to_array();
        }
        out
    }

    pub fn scaled(&self, k: f32) -> Sh9 {
        let mut out = *self;
        for c in &mut out.coeffs {
            *c = (Vec3::from(*c) * k).to_array();
        }
        out
    }

    pub fn band_scaled(&self, bands: [f32; 3]) -> Sh9 {
        let mut out = *self;
        for (i, c) in out.coeffs.iter_mut().enumerate() {
            let band = match i {
                0 => 0,
                1..=3 => 1,
                _ => 2,
            };
            *c = (Vec3::from(*c) * bands[band]).to_array();
        }
        out
    }

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

    #[test]
    fn directional_is_brighter_toward_source() {
        let mut sh = Sh9::zero();
        sh.add_sample(Vec3::Y, Vec3::splat(10.0), 1.0);
        let toward = sh.eval(Vec3::Y).x;
        let away = sh.eval(Vec3::NEG_Y).x;
        assert!(toward > away, "expected {toward} > {away}");
    }

    fn sphere_dirs() -> Vec<Vec3> {
        crate::lighting::probe_fill::fibonacci_sphere(512)
    }
}
