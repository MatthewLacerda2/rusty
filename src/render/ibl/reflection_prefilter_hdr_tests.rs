//! The prefilter in HDR (#1696): nothing is clamped, and a small bright source
//! blurs into a smooth lobe at high roughness rather than speckle.

use glam::Vec3;

use super::super::cpu::prefilter;
use super::face_variance;
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

/// At high roughness one small bright source is a smooth glow: neighbouring texels
/// of the face that sees it differ by a small part of the face's brightest, where
/// sampling only the base level would leave a speckle of the few taps that hit it.
#[test]
fn a_high_roughness_mip_of_a_small_lamp_is_smooth() {
    let res = 64;
    let cube = prefilter(&lamp_capture(res, 1000.0));
    let f = CubemapFace::PosZ as usize;
    for level in [3, 4] {
        let size = cube.level_size(level) as usize;
        let face = texels(&cube.mips[level][f]);
        let peak = face.iter().fold(0.0f32, |m, c| m.max(c.x));
        let step = (0..size * size)
            .filter(|i| i % size + 1 < size)
            .map(|i| (face[i].x - face[i + 1].x).abs())
            .fold(0.0f32, f32::max);
        assert!(peak > 0.0, "mip {level} sees the lamp");
        assert!(
            step < MAX_NEIGHBOUR_STEP * peak,
            "mip {level}: a neighbour step of {step} against a peak of {peak}"
        );
        assert!(face_variance(&cube.mips[level][f]).is_finite());
    }
}

/// The largest step between neighbouring texels of a rough mip, as a part of its
/// peak.
const MAX_NEIGHBOUR_STEP: f32 = 0.35;
