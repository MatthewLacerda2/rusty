//! RGBM: how a baked lightmap is stored (#438). An HDR texel fits an 8-bit RGBA PNG
//! as `rgb * m * RGBM_RANGE`, Unity's lightmap encoding on platforms without float
//! textures. No new dependency (the PNG encoder is already in), the file stays small,
//! and an agent can open it. The shader's `decode_rgbm` mirrors [`decode_rgbm`].
//!
//! Values are linear (no sRGB curve); the renderer uploads the PNG as `Rgba8Unorm`.
//!
//! A direction page (#810) is plain RGBA8, Unity's directional-lightmap layout: RGB the
//! dominant incoming direction mapped from [-1, 1] to [0, 1], A its directionality.
//! The shader's direction read mirrors [`decode_direction`].

use glam::Vec3;

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

/// Decode one RGBM texel back to linear HDR.
pub fn decode_rgbm(px: [u8; 4]) -> Vec3 {
    let f = |b: u8| b as f32 / 255.0;
    Vec3::new(f(px[0]), f(px[1]), f(px[2])) * f(px[3]) * RGBM_RANGE
}

/// A whole lightmap's texels as RGBA8 RGBM bytes, row-major.
pub fn encode_texels(texels: &[Vec3]) -> Vec<u8> {
    texels.iter().flat_map(|&c| encode_rgbm(c)).collect()
}

/// Encode one direction texel: a vector whose length is the directionality (0..=1).
pub fn encode_direction(d: Vec3) -> [u8; 4] {
    let a = d.length().min(1.0);
    let rgb = d.try_normalize().unwrap_or(Vec3::ZERO) * 0.5 + 0.5;
    let byte = |x: f32| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
    [byte(rgb.x), byte(rgb.y), byte(rgb.z), byte(a)]
}

/// Decode one direction texel back to the direction scaled by its directionality.
pub fn decode_direction(px: [u8; 4]) -> Vec3 {
    let f = |b: u8| b as f32 / 255.0;
    let dir = Vec3::new(f(px[0]), f(px[1]), f(px[2])) * 2.0 - 1.0;
    dir.try_normalize().unwrap_or(Vec3::ZERO) * f(px[3])
}

/// A whole direction page as RGBA8 bytes, row-major.
pub fn encode_directions(texels: &[Vec3]) -> Vec<u8> {
    texels.iter().flat_map(|&d| encode_direction(d)).collect()
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
            Vec3::Y,
            Vec3::new(-0.3, 0.2, 0.1),
            Vec3::new(0.6, -0.48, 0.64),
        ] {
            let back = decode_direction(encode_direction(d));
            assert!((back - d).length() < 0.02, "{d} -> {back}");
        }
        assert_eq!(decode_direction(encode_direction(Vec3::ZERO)), Vec3::ZERO);
        assert_eq!(encode_direction(Vec3::X * 3.0)[3], 255, "clamped to 1");
    }
}
