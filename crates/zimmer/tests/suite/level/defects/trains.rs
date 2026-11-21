//! Clicks that come in numbers: two close together, and one at every audio
//! buffer — the "choppy, like Bluetooth" a broken capture sounds like.
//!
//! A click is a jump with no twin around it, and a click's twin is not
//! another click: a sawtooth's next reset looks like this one, sample for
//! sample, where the next glitch in a broken stream lands at another point of
//! the waveform.

use super::{RATE, found, steady, tone};

#[test]
fn two_clicks_close_together_are_two_clicks() {
    for apart in [0.006, 0.009, 0.010, 0.020, 1679.0 / 48_000.0] {
        let mut signal = tone(440.0, 0.4, 2.0, steady);
        for at in [1.0, 1.0 + apart] {
            signal[(at * f64::from(RATE)) as usize] += 0.3;
        }
        let defects = found(1, &signal);
        assert_eq!(defects.clicks.count, 2, "{apart} s apart: {defects:?}");
    }
}

/// A stream that loses 37 samples after every 512-sample buffer: a splice
/// every 10.7 ms, each at a different point of the tone. Not every splice
/// clicks — one landing where the tone is barely moving is no jump — and at
/// a pitch whose cycle nearly divides the buffer (523 Hz: 5.98 cycles) each
/// splice repeats the last one and reads as part of the waveform.
#[test]
fn a_splice_at_every_buffer_is_a_click_at_every_buffer() {
    for size in [256, 512] {
        let whole = tone(470.0, 0.4, 3.5, steady);
        let signal: Vec<f32> = whole
            .chunks(size + 37)
            .flat_map(|buffer| buffer.iter().take(size).copied())
            .collect();
        let defects = found(1, &signal);
        let buffers = signal.len() / size;
        assert!(
            defects.clicks.count >= buffers * 3 / 4,
            "{} clicks in {buffers} buffers of {size}",
            defects.clicks.count
        );
    }
}
