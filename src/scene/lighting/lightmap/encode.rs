//! RGBM: how a baked lightmap's PNG fallback is stored (#438; the GPU reads BC6H
//! since #1694, `pages`). An HDR texel fits an 8-bit RGBA PNG
//! as `rgb * m * RGBM_RANGE`, Unity's lightmap encoding on platforms without float
//! textures. No new dependency (the PNG encoder is already in), the file stays small,
//! and an agent can open it.
//!
//! Values are linear (no sRGB curve). The renderer decodes the PNG on the CPU when
//! it loads it (a GPU without BC compression, or a scene baked before #1694).
//!
//! A direction page (#810) is plain RGBA8. RGB is the dominant incoming direction
//! scaled by its directionality, a vector of length 0..=1, each component stored as
//! `128 + 127·x` so 0 is exact; A is its **lamp share** (#1695): the part of the
//! texel's light that `Baked` lamps' direct light brings from one way,
//! `|direct_dir| / luminance(direct + indirect)` (`bake::direction_texel`), 0 where no
//! lamp reaches. Both are linear in the stored bytes, so bilinear filtering, the box
//! mips and BC7 average them as the vectors and shares they are (a packed bit field
//! would not survive), though BC7 lends a lamp-free texel a few steps of its block's
//! share, which the shader's `SHARE_FLOOR` ignores (`share_tests`). The shader's
//! direction read mirrors [`decode_direction`].

use glam::{Vec3, Vec4};

/// The brightest value RGBM can hold, per channel (Unity's range).
pub const RGBM_RANGE: f32 = 8.0;

/// Encode one linear HDR texel. Channels above [`RGBM_RANGE`] clamp; negatives read 0.
pub fn encode_rgbm(c: Vec3) -> [u8; 4] {
    let c = c.clamp(Vec3::ZERO, Vec3::splat(RGBM_RANGE));
    let peak = c.max_element() / RGBM_RANGE;
    // Round the multiplier up so `rgb = c / scale` never exceeds 1.
    let m = ((peak * 255.0).ceil() / 255.0).max(1.0 / 255.0);
    let rgb = c / (m * RGBM_RANGE);
    let byte = |x: f32| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
    [byte(rgb.x), byte(rgb.y), byte(rgb.z), byte(m)]
}

/// Decode one RGBM texel back to linear HDR: what a page's PNG fallback holds.
pub fn decode_rgbm(px: [u8; 4]) -> Vec3 {
    let f = |b: u8| b as f32 / 255.0;
    Vec3::new(f(px[0]), f(px[1]), f(px[2])) * f(px[3]) * RGBM_RANGE
}

/// A whole lightmap's texels as RGBA8 RGBM bytes, row-major.
pub fn encode_texels(texels: &[Vec3]) -> Vec<u8> {
    texels.iter().flat_map(|&c| encode_rgbm(c)).collect()
}

/// Encode one direction texel: `xyz` the direction scaled by its directionality
/// (clamped to length 1), `w` its lamp share (clamped to 0..=1).
pub fn encode_direction(d: Vec4) -> [u8; 4] {
    let v = d.truncate().clamp_length_max(1.0);
    let byte = |x: f32| (128.0 + 127.0 * x).round().clamp(1.0, 255.0) as u8;
    let share = (d.w.clamp(0.0, 1.0) * 255.0).round() as u8;
    [byte(v.x), byte(v.y), byte(v.z), share]
}

/// Decode one direction texel back to its vector and lamp share.
pub fn decode_direction(px: [u8; 4]) -> Vec4 {
    let f = |b: u8| (b as f32 - 128.0) / 127.0;
    Vec4::new(f(px[0]), f(px[1]), f(px[2]), px[3] as f32 / 255.0)
}

/// A whole direction page as RGBA8 bytes, row-major.
pub fn encode_directions(texels: &[Vec4]) -> Vec<u8> {
    texels.iter().flat_map(|&d| encode_direction(d)).collect()
}

/// One shadowmask texel (#1695) as RGBA8: each channel's occlusion, 0..=1, to a
/// byte. Unity's shadowmask is 8 bits a channel too.
pub fn encode_mask(mask: Vec4) -> [u8; 4] {
    mask.to_array()
        .map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8)
}

/// Decode one shadowmask texel back to its four occlusions.
pub fn decode_mask(px: [u8; 4]) -> Vec4 {
    Vec4::from_array(px.map(|b| b as f32 / 255.0))
}

/// A whole shadowmask page as RGBA8 bytes, row-major.
pub fn encode_masks(texels: &[Vec4]) -> Vec<u8> {
    texels.iter().flat_map(|&m| encode_mask(m)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_within_a_percent() {
        for c in [
            Vec3::new(0.01, 0.02, 0.03),
            Vec3::new(0.5, 0.25, 0.125),
            Vec3::new(3.0, 1.0, 0.2),
            Vec3::splat(7.9),
        ] {
            let back = decode_rgbm(encode_rgbm(c));
            let err = (back - c).abs().max_element();
            assert!(err <= c.max_element() * 0.01 + 1e-3, "{c} -> {back}");
        }
    }

    #[test]
    fn black_and_out_of_range_are_safe() {
        assert_eq!(decode_rgbm(encode_rgbm(Vec3::ZERO)), Vec3::ZERO);
        let clamped = decode_rgbm(encode_rgbm(Vec3::new(100.0, -1.0, 0.0)));
        assert!((clamped.x - RGBM_RANGE).abs() < 0.05 && clamped.y == 0.0);
    }

    #[test]
    fn directions_round_trip_and_none_stays_none() {
        for d in [
            Vec4::new(0.0, 1.0, 0.0, 1.0),
            Vec4::new(-0.3, 0.2, 0.1, 0.0),
            Vec4::new(0.6, -0.48, 0.64, 0.4),
        ] {
            let back = decode_direction(encode_direction(d));
            assert!((back - d).length() < 0.01, "{d} -> {back}");
        }
        assert_eq!(decode_direction(encode_direction(Vec4::ZERO)), Vec4::ZERO);
        let long = encode_direction(Vec4::new(3.0, 0.0, 0.0, 2.0));
        assert_eq!((long[0], long[3]), (255, 255), "clamped to 1");
        assert_eq!(encode_direction(-Vec4::X)[0], 1, "-1 is a byte");
    }

    #[test]
    fn a_page_averages_as_its_vectors_and_shares_do() {
        // What bilinear filtering and a mip do to the bytes, done to the values.
        let (a, b) = (
            Vec4::new(0.8, 0.2, 0.0, 1.0),
            Vec4::new(-0.2, 0.6, 0.4, 0.0),
        );
        let (ea, eb) = (encode_direction(a), encode_direction(b));
        let mid = std::array::from_fn(|k| ((ea[k] as u16 + eb[k] as u16) / 2) as u8);
        let back = decode_direction(mid);
        assert!((back - (a + b) * 0.5).length() < 0.01, "{back}");
    }
}
