//! How a light's tile size follows its screen coverage without flickering (#1711).
//! Pure CPU.
//!
//! A light's *settled* size is the one its coverage last earned. It changes only
//! after the coverage's wanted size has stayed out of the settled size's band for
//! [`SETTLE_FRAMES`] frames in a row, so a camera walking back and forth across a
//! step re-bakes nothing. The band is the size's step `[size, 2 × size)` widened by
//! [`BAND`] of a step (`size`) at each end, `[0.75 × size, 2.25 × size)`, and never
//! reaches past [`MAX_TILE`]: a want of `MAX_TILE` is out of band for every smaller
//! size, so a light the camera walks into always grows to the largest tile.
//!
//! The settled size is what the light asks of the budget; the budget may halve it
//! for a frame of pressure without touching it, so the tiles grow back the frame the
//! pressure ends.

use super::plan::{MAX_TILE, MIN_TILE};

/// How far past its size's step a light's wanted size may stray, as a fraction of
/// the step, before the size starts to change.
pub(super) const BAND: f32 = 0.25;
/// Frames in a row a light's wanted size must be out of [`BAND`] to change its size.
pub(super) const SETTLE_FRAMES: u32 = 15;

/// The size a light settled at `settled` should ask for this frame, wanting `want`
/// texels (`size` once rounded to a tile). `off_band` counts the frames in a row its
/// want has been out of band, and is updated here.
pub(super) fn settle(settled: u32, off_band: &mut u32, want: f32, size: u32) -> u32 {
    if in_band(settled, want) {
        *off_band = 0;
        return settled;
    }
    *off_band += 1;
    if *off_band >= SETTLE_FRAMES {
        size
    } else {
        settled
    }
}

/// Whether a light wanting `want` texels may keep tiles of `size`: `want` is within
/// [`BAND`] of a step past either end of `[size, 2 × size)`, open below
/// [`MIN_TILE`]'s step and above [`MAX_TILE`]'s, and below [`MAX_TILE`] otherwise.
pub(super) fn in_band(size: u32, want: f32) -> bool {
    let size_f = size as f32;
    let lo = if size <= MIN_TILE {
        0.0
    } else {
        size_f * (1.0 - BAND)
    };
    let hi = if size >= MAX_TILE {
        f32::INFINITY
    } else {
        (size_f * (2.0 + BAND)).min(MAX_TILE as f32)
    };
    (lo..hi).contains(&want)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_band_is_a_quarter_step_wide_and_stops_at_the_largest_tile() {
        assert!(in_band(256, 192.0) && !in_band(256, 191.9));
        assert!(in_band(128, 287.9) && !in_band(128, 288.0));
        assert!(in_band(256, 511.9) && !in_band(256, MAX_TILE as f32));
        assert!(in_band(MIN_TILE, 0.0) && in_band(MAX_TILE, 1e9));
    }
}
