//! A resident probe's uniform (#1697): its flags, and the luminance SH the shader's
//! `probe_normalisation` dots with the basis.

use glam::Vec3;

use super::{luminance_sh, ReflectionProbeUniform};
use rusty_core::color::luminance;
use rusty_scene::lighting::sh::Sh9;
use rusty_scene::ReflectionProbe;

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

/// The mirror's constants are the shader's.
#[test]
fn the_cpu_normalisation_mirrors_the_shaders_constants() {
    use super::{NORMALISE_BIAS, STORED_FLOOR};
    let wgsl = include_str!("../../../../../engine/shaders/forward/reflection.wgsl");
    for (name, value) in [
        ("STORED_FLOOR", STORED_FLOOR),
        ("NORMALISE_BIAS", NORMALISE_BIAS),
    ] {
        let line = format!("const {name}: f32 = {value:?};");
        assert!(wgsl.contains(&line), "{line}");
    }
}

/// One dominant lamp's L2 SH rings below zero for normals turned 104–145° from it:
/// there the stored value is floored at a share of its mean, so a pixel of modest
/// baked light is not saturated to 1 by the ringing (#1697).
#[test]
fn one_lamp_sh_ringing_does_not_saturate_the_scale() {
    let mut sh = Sh9::zero();
    sh.add_sample(Vec3::Y, Vec3::splat(40.0), 0.05);
    let packed = luminance_sh(sh);
    let n = Vec3::new(
        0.0,
        (125f32).to_radians().cos(),
        (125f32).to_radians().sin(),
    );
    assert!(shader_dot(packed, n) < 0.0, "the SH rings negative here");
    let mean = packed[0][0] * 0.282_094_8;
    let scale = super::normalisation(packed, n, mean * 0.1);
    assert!(scale < 0.5, "floored, not saturated: {scale}");
    assert_eq!(
        super::normalisation(packed, Vec3::Y, 10.0),
        1.0,
        "never above 1"
    );
}

/// The fallback is never box-projected (#1995), and the shape row carries the
/// projection's fade: the blend distance, then the volume's half-diagonal.
#[test]
fn the_fallback_is_never_box_projected_and_the_fade_is_packed() {
    let mut probe = ReflectionProbe::new(Vec3::ZERO, Vec3::new(3.0, 4.0, 0.0));
    probe.blend_distance = 2.0;
    let room = ReflectionProbeUniform::of(&probe, 0, 1, 0);
    assert_eq!(room.params[1], 1.0);
    assert_eq!(room.shape[2..], [2.0, 5.0]);
    probe.fallback = true;
    assert_eq!(ReflectionProbeUniform::of(&probe, 0, 1, 0).params[1], 0.0);
}
