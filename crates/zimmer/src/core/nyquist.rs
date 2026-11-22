//! Callers (`additive`, `fm::voicing`) compare a ratio with a strict `<`: a sine on Nyquist
//! alternates ±A at a level nobody chose.

/// Measured at the track's top, so a partial a vibrato lifts past Nyquist drops for the whole
/// note rather than clicking in and out.
pub(crate) fn ratio_ceiling(freqs: &[f32], sample_rate: f32) -> f32 {
    let top = freqs.iter().fold(0.0f32, |high, f| high.max(*f));
    if top > 0.0 {
        sample_rate * 0.5 / top
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 44_100.0;

    #[test]
    fn the_ceiling_follows_the_top_of_the_track() {
        assert_eq!(ratio_ceiling(&[100.0, 400.0, 200.0], RATE), 55.125);
        assert_eq!(ratio_ceiling(&[400.0], RATE), 55.125, "and only the top");
        assert_eq!(
            ratio_ceiling(&[100.0], RATE),
            220.5,
            "a lower note carries more of a series, which is what a real \
             instrument does"
        );
    }

    #[test]
    fn the_ceiling_is_half_the_rate_over_the_pitch() {
        assert_eq!(ratio_ceiling(&[100.0], 88_200.0), 441.0);
        assert_eq!(ratio_ceiling(&[100.0], 22_050.0), 110.25);
    }

    #[test]
    fn a_track_with_no_pitch_in_it_carries_nothing() {
        for track in [&[][..], &[0.0], &[0.0, -100.0], &[-1.0, -2.0]] {
            assert_eq!(ratio_ceiling(track, RATE), 0.0, "{track:?}");
        }
    }
}
