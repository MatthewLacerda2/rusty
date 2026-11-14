//! The source chain, the stored texels and the per-sample mip (#1696). GPU-free.

use glam::Vec3;

use super::*;
use rusty_core::color::f16_to_f32;

fn capture(res: u32, texel: impl Fn(usize, usize) -> Vec3) -> CubemapCapture {
    let face = |f| (0..(res * res) as usize).map(|i| texel(f, i)).collect();
    CubemapCapture {
        resolution: res,
        faces: std::array::from_fn(face),
    }
}

/// Each level halves the edge and box-averages the one above, down to 1×1, keeping
/// HDR values as they are: a bright texel's energy is spread, never clipped.
#[test]
fn the_chain_box_averages_down_to_one_texel_and_keeps_hdr() {
    let cube = capture(4, |_, i| Vec3::splat(if i == 0 { 160.0 } else { 0.0 }));
    let chain = source_chain(&cube);
    let edges: Vec<usize> = chain.iter().map(|l| l[0].len()).collect();
    assert_eq!(edges, [16, 4, 1]);
    assert_eq!(chain[0][3][0], Vec3::splat(160.0), "level 0 is the capture");
    assert_eq!(chain[1][3][0], Vec3::splat(40.0));
    assert_eq!(chain[2][3][0], Vec3::splat(10.0), "the mean of the face");
}

/// The chain holds the values the half-float texture will: rounded once, here.
#[test]
fn the_chain_is_rounded_to_half_floats() {
    let chain = source_chain(&capture(1, |_, _| Vec3::splat(0.1)));
    assert_eq!(chain[0][0][0].x, f16_to_f32(f32_to_f16(0.1)));
    assert_ne!(chain[0][0][0].x, 0.1);
}

/// A face's bytes are four little-endian halves per texel, alpha 1.
#[test]
fn encode_face_writes_rgba_halves() {
    let bytes = encode_face(&[Vec3::new(10.0, 0.5, 0.0)]);
    let half = |i: usize| f16_to_f32(u16::from_le_bytes([bytes[2 * i], bytes[2 * i + 1]]));
    assert_eq!(bytes.len(), TEXEL_BYTES);
    assert_eq!([half(0), half(1), half(2), half(3)], [10.0, 0.5, 0.0, 1.0]);
}

/// A sharp lobe's samples read the base; a wide lobe's read blurrier levels the
/// farther they fall from its peak; nothing reads past the chain.
#[test]
fn sample_mip_follows_the_lobe_and_stays_in_the_chain() {
    let base = 256;
    let sharp = (1.0f32 / 9.0).powi(4);
    assert!(
        sample_mip(1.0, sharp, 128, base) < 1.0,
        "a mirror-like peak"
    );
    assert_eq!(sample_mip(1.0, sharp / 100.0, 128, base), 0.0);
    let (peak, flank) = (
        sample_mip(1.0, 0.5, 128, base),
        sample_mip(0.3, 0.5, 128, base),
    );
    assert!(peak > 2.0 && flank > peak, "{peak} {flank}");
    assert_eq!(
        sample_mip(0.0, 1.0, 1, base),
        8.0,
        "clamped at the last level"
    );
}
