//! RGBM: how a baked lightmap is stored (#438). An HDR texel fits an 8-bit RGBA PNG
//! as `rgb * m * RGBM_RANGE`, Unity's lightmap encoding on platforms without float
//! textures. No new dependency (the PNG encoder is already in), the file stays small,
//! and an agent can open it. The shader's `decode_rgbm` mirrors [`decode_rgbm`].
//!
//! Values are linear (no sRGB curve); the renderer uploads the PNG as `Rgba8Unorm`.

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
}
