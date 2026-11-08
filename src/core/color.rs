//! src/core/color.rs — the colour maths more than one module needs (#939): the sRGB
//! transfer pair, Rec. 709 luminance, `#rrggbb` hex parsing and IEEE half floats
//! (the `Rgba16Float` texels a prefiltered cube face holds before BC6H, #1696,
//! #1885). Kept in `core` because `procgen`, `scene`,
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

/// `v` as an IEEE 754 half float, rounded to nearest even: past the largest half
/// (65504) it is infinite, below the smallest subnormal it is zero, a NaN stays a NaN.
pub fn f32_to_f16(v: f32) -> u16 {
    let bits = v.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let exp = ((bits >> 23) & 0xff) as i32;
    let mantissa = bits & 0x7f_ffff;
    if exp == 0xff {
        return sign | 0x7c00 | if mantissa != 0 { 0x200 } else { 0 };
    }
    let half_exp = exp - 127 + 15;
    if half_exp >= 0x1f {
        return sign | 0x7c00;
    }
    if half_exp <= 0 {
        // A subnormal half (or zero): the implicit one joins the mantissa.
        if half_exp < -10 {
            return sign;
        }
        let m = mantissa | 0x80_0000;
        let shift = (14 - half_exp) as u32;
        return sign | round_shift(m, shift) as u16;
    }
    // A carry out of the mantissa rounds into the exponent, as the format means it to.
    sign | (((half_exp as u32) << 10) + round_shift(mantissa, 13)) as u16
}

/// `m >> shift`, rounded to nearest, ties to even.
fn round_shift(m: u32, shift: u32) -> u32 {
    let (kept, rest, half) = (m >> shift, m & ((1 << shift) - 1), 1 << (shift - 1));
    kept + u32::from(rest > half || (rest == half && kept & 1 == 1))
}

/// The value of IEEE 754 half float `h`: normals, subnormals, zeros, infinities, NaN.
pub fn f16_to_f32(h: u16) -> f32 {
    let sign = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
    let exp = i32::from((h >> 10) & 0x1f);
    let frac = f32::from(h & 0x3ff);
    match exp {
        0 => sign * frac * 2f32.powi(-24),
        0x1f if frac == 0.0 => sign * f32::INFINITY,
        0x1f => f32::NAN,
        _ => sign * (1.0 + frac / 1024.0) * 2f32.powi(exp - 15),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_floats_round_trip_and_saturate_to_infinity() {
        for v in [0.0, 1.0, -2.5, 0.333, 10.0, 65504.0, 6.0e-8, 1.0e-5] {
            let back = f16_to_f32(f32_to_f16(v));
            assert!(
                (back - v).abs() <= v.abs() / 1024.0 + 6.0e-8,
                "{v} -> {back}"
            );
        }
        assert_eq!(f32_to_f16(1.0), 0x3c00);
        assert_eq!(f32_to_f16(65520.0), 0x7c00, "past the largest half");
        assert_eq!(f16_to_f32(f32_to_f16(100_000.0)), f32::INFINITY);
        assert!(f16_to_f32(f32_to_f16(f32::NAN)).is_nan());
        assert_eq!(f32_to_f16(1.0e-9), 0, "below the smallest subnormal");
        // Every half's value converts back to the same half.
        for h in (0..0x7c00u16).step_by(7) {
            assert_eq!(f32_to_f16(f16_to_f32(h)), h, "{h:#x}");
        }
    }

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
