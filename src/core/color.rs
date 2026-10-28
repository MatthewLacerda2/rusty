//! src/core/color.rs — the colour maths more than one module needs (#939): the sRGB
//! transfer pair, Rec. 709 luminance and `#rrggbb` hex parsing. Kept in `core` because `procgen`, `scene`,
//! `render` and `dev` all reach it. The shaders keep their own WGSL copies.

/// Inverse-sRGB transfer (IEC 61966-2-1) for one 8-bit channel, to linear `[0, 1]`.
pub fn srgb_to_linear(b: u8) -> f32 {
    gamma_to_linear(b as f32 / 255.0)
}

/// Inverse-sRGB transfer for one channel, unclamped: an HDR value above 1 follows
/// the curve's power segment, as Unity's `Mathf.GammaToLinearSpace` does.
pub fn gamma_to_linear(c: f32) -> f32 {
    if c <= 0.040_45 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Linear → sRGB transfer (IEC 61966-2-1) for one channel, clamped to `[0, 1]` first.
pub fn linear_to_srgb(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// Rec. 709 luminance of a linear RGB colour.
pub fn luminance(rgb: impl Into<[f32; 3]>) -> f32 {
    let [r, g, b] = rgb.into();
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// The bytes of `"#rrggbb"` / `"#rrggbbaa"`: RGB, and alpha when written. Every digit
/// must be a hex digit, since `u8::from_str_radix` alone also takes a sign (`"+f"`).
pub fn hex_bytes(hex: &str) -> Option<([u8; 3], Option<u8>)> {
    let digits = hex.strip_prefix('#')?;
    if !matches!(digits.len(), 6 | 8) || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&digits[2 * i..2 * i + 2], 16).ok();
    let alpha = if digits.len() == 8 {
        Some(byte(3)?)
    } else {
        None
    };
    Some(([byte(0)?, byte(1)?, byte(2)?], alpha))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_transfer_pair_round_trips_every_byte() {
        for b in 0..=255u8 {
            let back = (linear_to_srgb(srgb_to_linear(b)) * 255.0 + 0.5) as u8;
            assert_eq!(back, b);
        }
    }

    #[test]
    fn an_hdr_value_keeps_climbing_past_one() {
        assert_eq!(gamma_to_linear(1.0), 1.0);
        assert!(gamma_to_linear(2.0) > 4.0, "{}", gamma_to_linear(2.0));
    }

    #[test]
    fn luminance_weighs_green_most() {
        assert!((luminance([1.0, 1.0, 1.0]) - 1.0).abs() < 1e-6);
        assert!(luminance([0.0, 1.0, 0.0]) > luminance([1.0, 0.0, 1.0]));
    }

    #[test]
    fn hex_bytes_reads_rgb_and_optional_alpha() {
        assert_eq!(hex_bytes("#ff8000"), Some(([255, 128, 0], None)));
        assert_eq!(hex_bytes("#0000ff80"), Some(([0, 0, 255], Some(128))));
    }

    #[test]
    fn hex_bytes_refuses_signs_and_non_digits() {
        for bad in [
            "ff8000",
            "#ff800",
            "#ff800g",
            "#+f+f+f",
            "#-f0000",
            "#ééé",
            "#ff8000ff00",
        ] {
            assert_eq!(hex_bytes(bad), None, "{bad}");
        }
    }
}
