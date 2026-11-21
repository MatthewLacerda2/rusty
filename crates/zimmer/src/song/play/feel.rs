//! Swing and humanise — a song that was played, not clocked.

use serde::{Deserialize, Serialize};

use crate::hash::unit2;
use crate::song::{Song, Track};

const ONSET: u64 = 2;

const VELOCITY: u64 = 3;

const TIMBRE: u64 = 4;

const QUIETEST: f32 = 0.05;

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Humanize {
    #[serde(default)]
    pub timing: f32,
    #[serde(default)]
    pub velocity: f32,
    #[serde(default)]
    pub timbre: f32,
}

impl Humanize {
    pub(crate) fn onset_seconds(self, track: usize, ordinal: u64, song_seed: u64) -> f32 {
        if self.timing <= 0.0 {
            return 0.0;
        }
        self.timing * symmetric(ONSET, track, ordinal, song_seed)
    }

    pub(crate) fn velocity(self, written: f32, track: usize, ordinal: u64, song_seed: u64) -> f32 {
        if self.velocity <= 0.0 {
            return written;
        }
        let scale = 1.0 + self.velocity * symmetric(VELOCITY, track, ordinal, song_seed);
        written * scale.max(QUIETEST)
    }

    pub(crate) fn timbre(self, played: f32, track: usize, ordinal: u64, song_seed: u64) -> f32 {
        if self.timbre <= 0.0 {
            return 0.0;
        }
        played * self.timbre * symmetric(TIMBRE, track, ordinal, song_seed)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Played {
    pub(crate) swing: f32,
    pub(crate) humanize: Humanize,
    pub(crate) push: f32,
}

impl Played {
    pub(crate) fn of(song: &Song, track: &Track) -> Self {
        Self {
            swing: track.swing.unwrap_or(song.swing),
            humanize: track.humanize.unwrap_or_else(|| song.humanize()),
            push: track.push,
        }
    }
}

fn symmetric(channel: u64, track: usize, ordinal: u64, song_seed: u64) -> f32 {
    unit2(track as i64, ordinal as i64, channel, song_seed) * 2.0 - 1.0
}

pub(crate) fn swung(start: f32, swing: f32) -> f32 {
    if swing <= 0.0 {
        return start;
    }
    let beat = start.floor();
    let phase = start - beat;
    // Where the off-beat eighth now sits: half a beat, plus `swing` of the half-beat that remains
    // between it and the next downbeat.
    let late = 0.5 * (1.0 + swing);
    let swung = if phase <= 0.5 {
        phase * (1.0 + swing)
    } else {
        late + (phase - 0.5) * (1.0 - swing)
    };
    beat + swung
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn straight_is_the_identity_and_downbeats_never_move() {
        for start in [0.0, 0.5, 0.75, 3.25, 7.5] {
            assert_eq!(swung(start, 0.0), start);
        }
        for beat in [0.0, 1.0, 2.0, 16.0] {
            assert_eq!(swung(beat, 0.4), beat);
        }
    }

    #[test]
    fn the_off_beat_eighth_lands_where_the_documentation_says() {
        for (swing, expected) in [(0.0, 0.5), (0.5, 0.75), (1.0, 1.0)] {
            assert!((swung(0.5, swing) - expected).abs() < 1e-6);
            // And the same eighth in any later beat, by the same amount.
            assert!((swung(5.5, swing) - (5.0 + expected)).abs() < 1e-6);
        }
    }

    #[test]
    fn swing_never_reorders_the_notes() {
        for swing in [0.1, 0.33, 0.5, 0.9] {
            let mut previous = f32::NEG_INFINITY;
            for step in 0..64 {
                let now = swung(step as f32 / 8.0, swing);
                assert!(now > previous, "swing {swing} reordered at step {step}");
                previous = now;
            }
        }
    }

    #[test]
    fn humanising_stays_inside_the_band_it_was_given() {
        let feel = Humanize {
            timing: 0.02,
            velocity: 0.1,
            timbre: 0.0,
        };
        for ordinal in 0..256 {
            let seconds = feel.onset_seconds(0, ordinal, 9);
            assert!(seconds.abs() <= 0.02, "onset {seconds} escaped the band");
            let velocity = feel.velocity(0.5, 0, ordinal, 9);
            assert!((0.45..=0.55).contains(&velocity), "velocity {velocity}");
        }
    }

    #[test]
    fn an_extravagant_velocity_amount_still_cannot_reach_silence() {
        let wild = Humanize {
            timing: 0.0,
            velocity: 4.0,
            timbre: 0.0,
        };
        for ordinal in 0..256 {
            assert!(wild.velocity(0.5, 0, ordinal, 3) >= 0.5 * QUIETEST);
        }
    }

    #[test]
    fn a_timbre_nudge_is_bounded_by_the_velocity_it_scatters() {
        let feel = Humanize {
            timbre: 0.2,
            ..Humanize::default()
        };
        let mut moved = false;
        for ordinal in 0..256 {
            let loud = feel.timbre(1.0, 0, ordinal, 9);
            let soft = feel.timbre(0.25, 0, ordinal, 9);
            assert!(loud.abs() <= 0.2, "offset {loud} escaped the band");
            assert!((soft - loud * 0.25).abs() < 1e-6, "not a fraction of it");
            moved |= loud.abs() > 1e-6;
        }
        assert!(moved, "nothing was scattered at all");
    }

    #[test]
    fn nothing_is_drawn_when_nothing_was_asked_for() {
        let nothing = Humanize::default();
        assert_eq!(nothing.onset_seconds(0, 7, 1), 0.0);
        assert_eq!(nothing.velocity(0.3, 0, 7, 1), 0.3);
        assert_eq!(nothing.timbre(0.9, 0, 7, 1), 0.0);
    }

    #[test]
    fn the_three_axes_are_drawn_apart() {
        let tone_only = Humanize {
            timbre: 0.2,
            ..Humanize::default()
        };
        assert_eq!(tone_only.onset_seconds(0, 7, 1), 0.0);
        assert_eq!(tone_only.velocity(0.3, 0, 7, 1), 0.3);
        assert_ne!(tone_only.timbre(1.0, 0, 7, 1), 0.0);
    }
}
