//! The prefilter in HDR (#1696): nothing is clamped, and a small bright source
//! blurs into a smooth lobe at high roughness rather than speckle.

use glam::Vec3;

use super::cpu::{convolve, prefilter, Taps};
use super::source::source_chain;
use super::SAMPLE_COUNT;
use crate::render::ibl::cube_sample::{face_uv, FaceProjections};
use crate::render::ibl::reflection_prefilter::source::decode_face as texels;
use crate::render::{CubemapCapture, CubemapFace};

/// A black room with one small lamp of radiance `lamp`: a 2×2 patch at the centre
/// of the +Z face.
fn lamp_capture(res: u32, lamp: f32) -> CubemapCapture {
    let mut faces: [Vec<Vec3>; 6] = std::array::from_fn(|_| vec![Vec3::ZERO; (res * res) as usize]);
    for (x, y) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
        let (x, y) = (res / 2 - 1 + x, res / 2 - 1 + y);
        faces[CubemapFace::PosZ as usize][(y * res + x) as usize] = Vec3::splat(lamp);
    }
    CubemapCapture {
        resolution: res,
        faces,
    }
}

/// A lamp of radiance 10 is 10 in the stored mirror mip, and every rough mip keeps
/// light above 1 around it: nothing in the chain clamps at display white.
#[test]
fn an_hdr_lamp_keeps_its_radiance_in_the_chain() {
    let cube = prefilter(&lamp_capture(16, 10.0));
    let f = CubemapFace::PosZ as usize;
    let peak = |level: usize| {
        texels(&cube.mips[level][f])
            .iter()
            .fold(0.0f32, |m, c| m.max(c.x))
    };
    assert_eq!(peak(0), 10.0);
    assert!(
        peak(1) > 1.0,
        "the first rough mip still holds the lamp above 1: {}",
        peak(1)
    );
}

/// The energy of a small bright source survives a rough lobe: summed over the face
/// that sees it, the roughest mips hold about what the mirror mip does, so a lamp
/// does not vanish from rough metal.
#[test]
fn a_rough_mip_keeps_a_small_lamps_energy() {
    let res = 32;
    let cube = prefilter(&lamp_capture(res, 1000.0));
    let f = CubemapFace::PosZ as usize;
    let energy = |level: usize| {
        let size = cube.level_size(level) as f32;
        texels(&cube.mips[level][f])
            .iter()
            .map(|c| c.x)
            .sum::<f32>()
            / (size * size)
    };
    let (sharp, rough) = (energy(0), energy(3));
    assert!(
        rough > 0.3 * sharp,
        "the lamp's light is spread, not lost: {rough} vs {sharp}"
    );
}

/// At high roughness one small bright source is a smooth glow: the bake's 128
/// filtered taps land close to a 16384-tap ground truth over the face that sees it,
/// where 128 taps of the base level alone leave a speckle of the few that hit it.
#[test]
fn a_high_roughness_mip_of_a_small_lamp_is_smooth() {
    let res = 64;
    let capture = lamp_capture(res, 1000.0);
    let cube = prefilter(&capture);
    let (proj, chain) = (FaceProjections::new(), source_chain(&capture));
    let levels = cube.level_count();
    for level in [2, 3, 4] {
        let size = cube.level_size(level);
        let roughness = level as f32 / (levels - 1) as f32;
        let at = |i: u32, taps: Taps| {
            let (u, v) = face_uv(i % size, i / size, size);
            let n = proj.direction(CubemapFace::PosZ, u, v);
            convolve(&proj, &chain, res, n, (roughness, taps)).x
        };
        let truth: Vec<f32> = (0..size * size).map(|i| at(i, TRUTH)).collect();
        let peak = truth.iter().fold(0.0f32, |m, &v| m.max(v));
        let error = |got: &dyn Fn(u32) -> f32| {
            let squares = (0..size * size).map(|i| (got(i) - truth[i as usize]).powi(2));
            (squares.sum::<f32>() / (size * size) as f32).sqrt() / peak
        };
        let baked = texels(&cube.mips[level][CubemapFace::PosZ as usize]);
        let filtered = error(&|i| baked[i as usize].x);
        let base_only = error(&|i| at(i, BASE_ONLY));
        assert!(
            filtered < MAX_ERROR,
            "mip {level}: {filtered} of the peak from the truth"
        );
        assert!(
            filtered < base_only / 2.0,
            "mip {level}: filtered {filtered} vs base-only {base_only}"
        );
    }
}

/// The ground truth's taps: enough that every one of the lamp's texels is hit often.
const TRUTH: Taps = Taps {
    count: 16384,
    filtered: false,
};
/// The bake's tap count without the per-tap mip: the speckle filtering removes.
const BASE_ONLY: Taps = Taps {
    count: SAMPLE_COUNT,
    filtered: false,
};
/// The bake's root-mean-square distance from the truth, as a part of the truth's peak.
/// Measured: 0.014, 0.051 and 0.177 at mips 2–4, against 0.099, 0.50 and 0.63 for
/// the base level alone. The roughest mips (5 and 6 here, two texels and one) stay
/// about 0.4 off: their taps read the 1×1 and 2×2 source levels by nearest texel,
/// which a bilinear lookup across face edges would smooth.
const MAX_ERROR: f32 = 0.25;
