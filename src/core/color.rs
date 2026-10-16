//! src/core/color.rs — the colour maths more than one module needs (#939): the sRGB
//! transfer pair and Rec. 709 luminance. Kept in `core` because `procgen`, `scene`,
//! `render` and `dev` all reach it. The shaders keep their own WGSL copies.

/// Inverse-sRGB transfer (IEC 61966-2-1) for one 8-bit channel, to linear `[0, 1]`.
pub fn srgb_to_linear(b: u8) -> f32 {
    let c = b as f32 / 255.0;
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
    fn luminance_weighs_green_most() {
        assert!((luminance([1.0, 1.0, 1.0]) - 1.0).abs() < 1e-6);
        assert!(luminance([0.0, 1.0, 0.0]) > luminance([1.0, 0.0, 1.0]));
    }
}
