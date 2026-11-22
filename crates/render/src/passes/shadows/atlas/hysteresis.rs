use super::plan::{MAX_TILE, MIN_TILE};

pub(super) const BAND: f32 = 0.25;
pub(super) const SETTLE_FRAMES: u32 = 15;

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
