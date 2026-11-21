//! A delay timed in beats: the spacing read off the tempo, and following it
//! when the tempo moves.

use super::super::*;
use crate::fx::NoTempo;

const RATE: f32 = 8_000.0;

/// A tempo that plays one beat length up to a frame and another after it —
/// the smallest tempo map there is.
struct Steps {
    until: usize,
    before: f32,
    after: f32,
}

impl Tempo for Steps {
    fn beat_seconds(&self, frame: usize) -> f32 {
        if frame < self.until {
            self.before
        } else {
            self.after
        }
    }
    fn longest_beat(&self) -> f32 {
        self.before.max(self.after)
    }
}

/// Where the non-zero samples of a fully wet line are.
fn echoes(buf: &[f32]) -> Vec<usize> {
    (0..buf.len()).filter(|i| buf[*i].abs() > 1e-6).collect()
}

/// Half a beat at 120 bpm is a quarter of a second, and is exactly what
/// `time: 0.25` would have played.
#[test]
fn a_beat_delay_at_one_tempo_is_the_seconds_it_works_out_to() {
    let tempo = Steps {
        until: usize::MAX,
        before: 0.5,
        after: 0.5,
    };
    let mut beats = vec![0.0; 8_000];
    beats[0] = 1.0;
    let mut seconds = beats.clone();
    apply(&mut beats, Spacing::Beats(0.5, &tempo), 0.5, 1.0, RATE);
    apply(&mut seconds, Spacing::Seconds(0.25), 0.5, 1.0, RATE);
    assert_eq!(beats, seconds);
    assert_eq!(echoes(&beats), [2_000, 4_000, 6_000]);
}

/// The tempo doubles at frame 3000: the first echo is spaced at the old beat,
/// and every one sent after the change at the new one — on the grid either
/// side of it, which a fixed `time` cannot be.
#[test]
fn the_spacing_follows_the_tempo_where_it_changes() {
    let tempo = Steps {
        until: 3_000,
        before: 0.5,
        after: 0.25,
    };
    let mut buf = vec![0.0; 8_000];
    buf[0] = 1.0;
    apply(&mut buf, Spacing::Beats(0.5, &tempo), 0.5, 1.0, RATE);
    assert_eq!(echoes(&buf), [2_000, 3_000, 4_000, 5_000, 6_000, 7_000]);
    // And the same for the pair, whose lines are read the same way.
    let mut wide = Stereo::centred({
        let mut mono = vec![0.0; 8_000];
        mono[0] = 1.0;
        mono
    });
    ping_pong(&mut wide, Spacing::Beats(0.5, &tempo), 0.5, 1.0, RATE);
    assert_eq!(echoes(&wide.l), [2_000, 4_000, 6_000]);
    assert_eq!(echoes(&wide.r), [3_000, 5_000, 7_000]);
}

/// Beats with no tempo to read them against — a patch's chain, which refuses
/// them in validation — are no delay at all, like a `time` of zero.
#[test]
fn beats_without_a_tempo_are_a_no_op() {
    let mut buf = vec![0.0; 1_000];
    buf[0] = 1.0;
    let before = buf.clone();
    apply(&mut buf, Spacing::Beats(0.5, &NoTempo), 0.5, 0.5, RATE);
    assert_eq!(buf, before);
    assert_eq!(Spacing::Beats(0.5, &NoTempo).longest(), 0.0);
    assert_eq!(Spacing::new(0.3, None, &NoTempo).longest(), 0.3);
}
