//! what a note may place above the pitch it was played at.
//!
//! Two sources build a tone out of sines placed at multiples of the played
//! pitch: [`additive`](super::additive) states a series of them outright, and
//! [`fm::four`](super::fm::four) wires four of them into each other. Both then
//! face the same question about each one — **is a sine at `ratio × pitch`
//! something this buffer can hold?** — and both have to answer it before they
//! render anything, because a sine past half the sample rate is not a bright
//! partial but an aliased one at some unrelated frequency.
//!
//! The two do different things with the answer. A dropped partial simply
//! leaves the sum; a dropped operator leaves the sum *and* stops bending
//! whatever it was modulating. But the question is one question, and it is
//! asked here so that the three decisions inside it are made once:
//!
//! - **Against the highest frequency the pitch track reaches**, not the note's
//!   nominal pitch. A partial that would cross the line at the top of a
//!   vibrato is dropped for the whole note rather than blinking in and out of
//!   it, which would be an audible click at every crossing.
//! - **Once per note**, not per patch — which multiples are legal depends on
//!   the pitch played, so the same organ is a full series in the bass and a
//!   handful at the top of the keyboard, which is also what a real one does.
//! - **A track that never rises above zero has no pitch to place anything
//!   against**, so nothing sounds: a ceiling of zero drops every one of them.
//!
//! Whoever drops an entry must also leave it out of their normalisation. That
//! part belongs to the caller and both of them say so, but it is the same
//! reason twice: a source whose level fell away as it was played higher,
//! purely because more of it had gone past Nyquist, would fight every gain
//! decision the mix makes.

/// The largest ratio a sine may take and still stay below Nyquist for a note
/// whose pitch track is `freqs`.
///
/// Compare a ratio against it with a **strict** `<`, and deliberately so. A
/// sine landing exactly on Nyquist advances exactly half a cycle per sample,
/// so it renders as an alternating ±A whose amplitude is decided by wherever
/// in its cycle the note happened to start — no converter can reproduce it and
/// nobody chose its level, which makes it the same unusable signal as one that
/// folded.
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

    /// The rate everything renders at, as the DSP sees it.
    const RATE: f32 = 44_100.0;

    /// The ceiling follows the **top** of the track, so a note that is bent
    /// upward part-way through has been counted against its highest moment
    /// from the first sample.
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

    /// Twice the rate is twice the ceiling: the bound is half the sample rate
    /// over the pitch, not a constant that happens to be right at 44.1 kHz.
    #[test]
    fn the_ceiling_is_half_the_rate_over_the_pitch() {
        assert_eq!(ratio_ceiling(&[100.0], 88_200.0), 441.0);
        assert_eq!(ratio_ceiling(&[100.0], 22_050.0), 110.25);
    }

    /// No pitch to place anything against is a ceiling of zero, which drops
    /// everything — a strict `<` against it is false for every ratio there is,
    /// including zero itself.
    #[test]
    fn a_track_with_no_pitch_in_it_carries_nothing() {
        for track in [&[][..], &[0.0], &[0.0, -100.0], &[-1.0, -2.0]] {
            assert_eq!(ratio_ceiling(track, RATE), 0.0, "{track:?}");
        }
    }
}
