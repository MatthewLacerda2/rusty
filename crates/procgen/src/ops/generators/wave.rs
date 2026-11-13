//! Bands and rings (#392) with Blender's Wave Texture profiles and distortion
//! (#1740). The phase is in cycles, `frequency` whole ones across the tile, so every
//! profile closes at the wrap; the distortion is a tiling fBM, so it does too.

use std::f32::consts::TAU;

use super::{count, finite, gray, noise, Sampler};
use crate::catalog::generators::wave::WaveProfile;
use crate::recipe::WaveKind;

/// The largest `distortion`, in radians either way: far past the point where the
/// bands are lost in noise, and finite.
pub const MAX_DISTORTION: f32 = 1000.0;

/// The distortion noise's seed salt, so it is not the `noise` op's field at the
/// same seed and lattice.
const DISTORTION_SALT: u64 = 0x85eb_ca6b;

/// The most `detail` reads: Blender's Wave Texture caps it at 15 too.
pub const MAX_DETAIL: u32 = 15;

/// The wave sampler: `frequency` cycles of `profile` across the tile, as bands
/// (along `u`) or rings (from the centre), the phase pushed by `distortion` radians
/// of an fBM with `detail + 1` octaves at the wave's own lattice. NaN distortion
/// reads 0 and 0 skips the noise, so a plain sine bakes as it always did.
pub fn sampler(
    kind: WaveKind,
    frequency: f32,
    (profile, distortion, detail): (WaveProfile, f32, u32),
    seed: u64,
) -> Sampler<'static> {
    let f = count(frequency);
    let push = finite(distortion, 0.0).clamp(-MAX_DISTORTION, MAX_DISTORTION) / TAU;
    let octaves = detail.min(MAX_DETAIL) + 1;
    Box::new(move |u, v| {
        let mut phase = match kind {
            WaveKind::Bands => u * f,
            WaveKind::Rings => (u - 0.5).hypot(v - 0.5) * f,
        };
        if push != 0.0 {
            phase += push * noise::signed_fbm(u, v, f, octaves, seed ^ DISTORTION_SALT);
        }
        gray(shape(profile, phase))
    })
}

/// One cycle of `profile` at `phase` (in cycles), in `[0, 1]`. The three agree at
/// whole cycles (0.5, rising) and peak together a quarter in.
fn shape(profile: WaveProfile, phase: f32) -> f32 {
    match profile {
        WaveProfile::Sine => 0.5 + 0.5 * (phase * TAU).sin(),
        WaveProfile::Saw => (phase + 0.5).rem_euclid(1.0),
        WaveProfile::Triangle => 1.0 - (2.0 * (phase + 0.25).rem_euclid(1.0) - 1.0).abs(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiles_agree_at_whole_cycles_and_peak_together() {
        for profile in [WaveProfile::Sine, WaveProfile::Saw, WaveProfile::Triangle] {
            assert!((shape(profile, 2.0) - 0.5).abs() < 1e-5, "{profile:?}");
        }
        assert!((shape(WaveProfile::Sine, 0.25) - 1.0).abs() < 1e-6);
        assert!((shape(WaveProfile::Triangle, 0.25) - 1.0).abs() < 1e-6);
        assert!((shape(WaveProfile::Triangle, 0.75)).abs() < 1e-6);
        assert!(
            shape(WaveProfile::Saw, 0.49) > 0.98,
            "a saw climbs to the drop"
        );
        assert!(shape(WaveProfile::Saw, 0.51) < 0.02, "and falls at it");
    }

    #[test]
    fn a_malformed_distortion_or_detail_is_bounded() {
        for d in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 1e30] {
            let f = sampler(WaveKind::Bands, 3.0, (WaveProfile::Sine, d, u32::MAX), 1);
            let x = f(0.3, 0.7)[0];
            assert!((0.0..=1.0).contains(&x), "distortion {d}: {x}");
        }
        let nan = sampler(WaveKind::Bands, 3.0, (WaveProfile::Saw, f32::NAN, 2), 1);
        let zero = sampler(WaveKind::Bands, 3.0, (WaveProfile::Saw, 0.0, 2), 1);
        assert_eq!(nan(0.3, 0.7), zero(0.3, 0.7), "NaN distortion is none");
    }
}
