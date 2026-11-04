//! A resident probe's uniform (#1697): its flags, and the luminance SH the shader's
//! `probe_normalisation` dots with the basis.

use glam::Vec3;

use super::{luminance_sh, ReflectionProbeUniform};
use crate::core::color::luminance;
use crate::scene::lighting::sh::Sh9;
use crate::scene::ReflectionProbe;

/// What the shader computes from the packed coefficients along `n`: their dot
/// product with the basis, which is `eval_radiance` of an SH holding them.
fn shader_dot(packed: [[f32; 4]; 3], n: Vec3) -> f32 {
    let flat = packed.as_flattened();
    let sh = Sh9 {
        coeffs: std::array::from_fn(|i| [flat[i]; 3]),
    };
    sh.eval_radiance(n).x
}

#[test]
fn the_packed_luminance_sh_reads_back_the_irradiance_luminance_over_pi() {
    let mut sh = Sh9::zero();
    sh.add_sample(Vec3::Y, Vec3::new(3.0, 2.0, 1.0), 4.0);
    sh.add_sample(Vec3::X, Vec3::new(0.5, 1.0, 2.0), 2.0);
    for n in [
        Vec3::Y,
        Vec3::X,
        -Vec3::Z,
        Vec3::new(1.0, 1.0, 0.0).normalize(),
    ] {
        let want = luminance(sh.eval(n)) / std::f32::consts::PI;
        let got = shader_dot(luminance_sh(sh), n);
        assert!((got - want).abs() < 1e-4, "{n}: {got} vs {want}");
    }
}

#[test]
fn a_probe_without_stored_irradiance_is_not_normalised_and_the_fallback_is_flagged() {
    let mut probe = ReflectionProbe::new(Vec3::ZERO, Vec3::ONE);
    let plain = ReflectionProbeUniform::of(&probe, 0, 1, 0);
    assert_eq!(plain.params[2..], [0.0, 0.0]);
    assert_eq!(plain.irradiance, [[0.0; 4]; 3]);
    probe.irradiance = Some(Sh9::zero());
    probe.fallback = true;
    let baked = ReflectionProbeUniform::of(&probe, 0, 1, 0);
    assert_eq!(baked.params[2..], [1.0, 1.0]);
}
