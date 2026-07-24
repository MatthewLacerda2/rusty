//! src/soundgen/fx/delay.rs — feedback echo (#357).
//!
//! A ring buffer one echo-time long, read and written each sample: what comes out
//! is fed back in, scaled, so each repeat is quieter than the last. Cheap, and it is
//! the effect that most reliably makes a dry one-shot sound like it happened
//! *somewhere* — a slapback on a gunshot, a corridor on a footstep.
//!
//! The wet signal is blended with the dry one by `mix`, so a delay is always a
//! parallel effect, never a replacement.

/// Apply a feedback delay to `buf` in place.
///
/// `time` is the echo spacing in seconds, `feedback` how much of each echo feeds
/// the next (clamped below 1 so the tail always dies), `mix` the wet/dry blend.
pub fn apply(buf: &mut [f32], time: f32, feedback: f32, mix: f32, rate: f32) {
    if !time.is_finite() || time <= 0.0 {
        return;
    }
    let len = ((time * rate).round() as usize).max(1);
    // An echo far longer than the note itself would never be heard; skip rather
    // than allocate a delay line nobody reads.
    if len >= buf.len().max(1) * 8 {
        return;
    }
    let feedback = feedback.clamp(0.0, 0.95);
    let mix = mix.clamp(0.0, 1.0);
    let mut line = vec![0.0f32; len];
    let mut read = 0usize;
    for s in buf.iter_mut() {
        let echo = line[read];
        line[read] = *s + echo * feedback;
        read = (read + 1) % len;
        *s = *s * (1.0 - mix) + echo * mix;
    }
}

/// How long the echoes keep ringing after the dry signal stops, in seconds — what
/// the renderer pads the buffer by so the tail is not cut off mid-repeat.
pub fn tail_seconds(time: f32, feedback: f32) -> f32 {
    let time = time.clamp(0.0, 4.0);
    let feedback = feedback.clamp(0.0, 0.95);
    if feedback <= 0.0 {
        return time;
    }
    // Repeats until the echo is 60 dB down, i.e. feedback^n = 0.001.
    let repeats = (0.001f32.ln() / feedback.ln()).ceil().clamp(1.0, 32.0);
    (time * repeats).min(4.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A single full-scale impulse followed by silence.
    fn impulse(n: usize) -> Vec<f32> {
        let mut buf = vec![0.0; n];
        buf[0] = 1.0;
        buf
    }

    #[test]
    fn the_echo_lands_one_delay_time_later_and_decays() {
        let mut buf = impulse(44_100);
        apply(&mut buf, 0.1, 0.5, 1.0, 44_100.0);
        // Fully wet: the dry impulse is gone and echoes appear every 4410 samples.
        assert!(buf[0].abs() < 1e-6, "fully wet keeps no dry signal");
        assert!((buf[4410] - 1.0).abs() < 1e-6, "first echo at 100 ms");
        assert!((buf[8820] - 0.5).abs() < 1e-6, "second echo, halved");
        assert!((buf[13230] - 0.25).abs() < 1e-6, "third echo, halved again");
    }

    #[test]
    fn mix_blends_wet_against_dry() {
        let mut buf = impulse(22_050);
        apply(&mut buf, 0.1, 0.0, 0.25, 44_100.0);
        assert!((buf[0] - 0.75).abs() < 1e-6, "dry is scaled by 1 - mix");
        assert!((buf[4410] - 0.25).abs() < 1e-6, "echo is scaled by mix");
    }

    #[test]
    fn runaway_feedback_is_clamped_so_the_tail_always_dies() {
        let mut buf = impulse(44_100);
        apply(&mut buf, 0.01, 5.0, 1.0, 44_100.0);
        assert!(buf.iter().all(|s| s.abs() <= 1.0 + 1e-6), "no runaway");
        let last = buf[40_000..].iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(last < 0.2, "the tail decayed to {last}");
    }

    #[test]
    fn a_degenerate_time_is_a_no_op_rather_than_a_panic() {
        let original = impulse(1024);
        for time in [0.0, -1.0, f32::INFINITY, 1000.0] {
            let mut buf = original.clone();
            apply(&mut buf, time, 0.5, 0.5, 44_100.0);
            assert!(buf.iter().all(|s| s.is_finite()), "time {time}");
        }
    }

    #[test]
    fn tail_length_covers_the_audible_repeats() {
        assert!(
            (tail_seconds(0.25, 0.0) - 0.25).abs() < 1e-6,
            "one echo only"
        );
        assert!(tail_seconds(0.25, 0.5) >= 2.0, "10 repeats at 60 dB down");
        assert!(tail_seconds(3.0, 0.9) <= 4.0, "but bounded");
    }
}
