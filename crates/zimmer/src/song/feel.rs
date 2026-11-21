//! Swing and humanise — a song that was played, not clocked.
//!
//! Every note in [`super::render`] lands on the sample its beat works out to,
//! at exactly the velocity written, forever. That is *correct*, and it is also
//! the loudest signal that a piece of music was generated rather than
//! performed — louder than the notes themselves, and it survives every
//! improvement made to the composition.
//!
//! Two different things fix it, and keeping them apart is the point:
//!
//! - **Swing** is a decision. The off-beat eighths sit late, by the same
//!   amount every time, because almost nothing outside classical music is
//!   played on straight eighths. It is a feel, not an error.
//! - **Humanise** is the error. Each onset a little early or late, each note a
//!   little harder or softer and a little brighter or duller, never twice the
//!   same.
//!
//! They compose in that order — swing says where the beat *is*, humanise says
//! how well the player hit it — and both are absent by default. An absent
//! field renders byte-identical bytes to a song written before either existed,
//! which is not politeness: `generated/` addresses a bake by the hash of its
//! recipe, so a global change in where notes are placed would leave the cache
//! serving stale audio under unchanged hashes.
//!
//! **The song's, unless a track says otherwise.** Swing and humanise are
//! written once for the piece, and a track may write either again for itself
//! — a shuffled hat over a straight kick, a tight kick under loose keys. A
//! third field is a track's only: `push`, the seconds a part sits off
//! everyone else's beat. It is a decision like swing, not an error like
//! humanise — a snare laid back the same amount every time is where a
//! record's groove lives — and it lands last, after the player's scatter.
//! [`Played`] resolves the three per track.
//!
//! **Seeded, never random.** Humanisation draws from the same seeded integer
//! hash everything stochastic here draws from, at the same `(track, ordinal)`
//! coordinates the note's own render seed comes from — no `rand`, no wall
//! clock. A humanised song is as byte-identical across runs as a rigid one,
//! and re-rolling the whole performance is still one number: the song's seed.

use serde::{Deserialize, Serialize};

use super::{Song, Track};
use crate::hash::unit2;

/// The channel of the per-note hash an onset nudge is drawn on.
///
/// [`note_seed`](super::render) spends channels 0 and 1 on the seed the note
/// renderer itself uses, so 2 and 3 are the next free ones. Distinct channels
/// are what keep a note's timing, its velocity and its noise from moving
/// together — the reason [`hash3`](crate::hash::hash3) takes the argument at
/// all.
const ONSET: u64 = 2;

/// The channel of the per-note hash a velocity nudge is drawn on.
const VELOCITY: u64 = 3;

/// The channel of the per-note hash a timbre nudge is drawn on.
///
/// Its own channel rather than the velocity one, or the two would move
/// together — and a brightness that tracks the level exactly is the single
/// hand this field exists to split in two.
const TIMBRE: u64 = 4;

/// The quietest a humanised note may be pushed, as a fraction of the velocity
/// written.
///
/// A `velocity` amount above 1 could otherwise reach zero — a note that simply
/// vanished, which reads as a bug in the pattern rather than as feel — and past
/// it a *negative* scale, which is a phase inversion wearing "quieter" as a
/// disguise. A performance has soft notes; it does not have missing ones.
const QUIETEST: f32 = 0.05;

/// How far a player strays from the written page.
///
/// Every field is a magnitude: how wide the scatter is, either way. Absent
/// means a machine plays the part, which is what happened before this existed.
///
/// Three axes, because a player has three: *when* the note lands, *how hard*
/// it is struck, and *how it is struck* — the tone that comes of bow pressure,
/// pick angle, how squarely a key is hit. The third is not a shade of the
/// second: leaning on a note makes it louder **and** brighter together, which
/// is `velocity`; changing the touch makes it brighter at the same level,
/// which is `timbre`.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Humanize {
    /// The widest an onset may be nudged, either way, **in seconds**.
    ///
    /// Seconds rather than beats because human timing error does not scale
    /// with tempo — a player is not three times sloppier at 40 bpm than at 120,
    /// and a figure that is tight at one tempo should not turn sloppy when the
    /// piece is slowed down. Ten milliseconds is a tight band; forty is loose
    /// enough to hear as a person.
    #[serde(default)]
    pub timing: f32,
    /// The widest a velocity may be scaled, either way, as a fraction of what
    /// was written: `0.08` plays each note between 92% and 108% of its written
    /// velocity.
    ///
    /// A note written at full velocity can only come out *quieter*, because
    /// full is full and the note renderer clamps there. A song that wants
    /// dynamics in both directions writes its notes below 1.0 — which is what
    /// a score does anyway.
    #[serde(default)]
    pub velocity: f32,
    /// The widest a note's *brightness* may stray from its level, as a
    /// fraction of the velocity it ends up played at: `0.15` shows the
    /// brightness routings a velocity between 85% and 115% of the one the
    /// fader got, and moves the fader by none of it.
    ///
    /// It reaches a patch through the two routings that already read velocity
    /// as effort — a filter's `vel_octaves` and two-operator FM's `vel_index`
    /// — so a patch that names neither hears nothing from this field. That is
    /// the right silence rather than a gap: what "brighter" means belongs to
    /// the instrument, and an instrument that never said is not one to guess
    /// for.
    #[serde(default)]
    pub timbre: f32,
}

impl Humanize {
    /// True when no field does anything, so nothing has to be drawn and the
    /// render is what it was before the fields existed.
    // Its own test is the only caller outside a test build: this is the
    // shortcut a renderer takes before drawing per-note nudges, and nothing
    // takes it yet. Silent until `Humanize` stopped being reachable from
    // outside the crate; the expectation lifts itself the moment one does.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "no caller yet outside its own test")
    )]
    pub(crate) fn is_nothing(self) -> bool {
        self.timing <= 0.0 && self.velocity <= 0.0 && self.timbre <= 0.0
    }

    /// Seconds to shift one note's onset by — positive is late.
    ///
    /// The caller adds this to where the beat put the note and clamps the
    /// result at the start of the buffer: a note nudged early on the very first
    /// beat has nowhere to go, and a negative sample index is not a time.
    pub(crate) fn onset_seconds(self, track: usize, ordinal: u64, song_seed: u64) -> f32 {
        if self.timing <= 0.0 {
            return 0.0;
        }
        self.timing * symmetric(ONSET, track, ordinal, song_seed)
    }

    /// The velocity to actually play a note at, given what the document asks
    /// for.
    ///
    /// A scatter *around* what was written, never a decision about it: the
    /// written value has already been through the arrangement's `vel_scale`,
    /// and humanising is not the stage that gets to overrule either.
    pub(crate) fn velocity(self, written: f32, track: usize, ordinal: u64, song_seed: u64) -> f32 {
        if self.velocity <= 0.0 {
            return written;
        }
        let scale = 1.0 + self.velocity * symmetric(VELOCITY, track, ordinal, song_seed);
        written * scale.max(QUIETEST)
    }

    /// How far to move the velocity the *brightness* routings see, given the
    /// velocity `played` the note is actually struck at.
    ///
    /// An offset rather than a replacement, because the note renderer adds it
    /// to the played velocity and clamps the pair once — a second scale
    /// applied in a second place is a second chance to clamp differently. A
    /// fraction of the played velocity rather than of full scale, so a note
    /// struck softly strays less in absolute terms than one struck hard, which
    /// is what a hand does.
    pub(crate) fn timbre(self, played: f32, track: usize, ordinal: u64, song_seed: u64) -> f32 {
        if self.timbre <= 0.0 {
            return 0.0;
        }
        played * self.timbre * symmetric(TIMBRE, track, ordinal, song_seed)
    }
}

/// How one track is played: the song's swing and humanise unless the track
/// writes its own, and where it sits against the beat.
///
/// A track's own field **replaces** the song's rather than adding to it — a
/// tight kick under loose keys is a kick with less scatter, which a sum could
/// not write — and a track that writes neither plays exactly what it did
/// before the fields existed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Played {
    /// Where its off-beat eighths sit, as [`swung`] reads it.
    pub(crate) swing: f32,
    /// How far its player strays.
    pub(crate) humanize: Humanize,
    /// Seconds it sits off the beat, positive late.
    pub(crate) push: f32,
}

impl Played {
    /// How `track` of `song` is played.
    pub(crate) fn of(song: &Song, track: &Track) -> Self {
        Self {
            swing: track.swing.unwrap_or(song.swing),
            humanize: track.humanize.unwrap_or_else(|| song.humanize()),
            push: track.push,
        }
    }
}

/// A deterministic draw in `-1.0..1.0` for one note on one channel.
///
/// The coordinates are exactly the ones the note's render seed is derived from
/// — `(track, ordinal)` under the song's seed — and the ordinal counts notes in
/// *arrangement* order, so a pattern played twice is humanised differently on
/// each pass. That is the whole point rather than a side effect: eight bars
/// nudged identically both times round is still a photocopy, just a crooked
/// one.
fn symmetric(channel: u64, track: usize, ordinal: u64, song_seed: u64) -> f32 {
    unit2(track as i64, ordinal as i64, channel, song_seed) * 2.0 - 1.0
}

/// Where a note written at `start` beats actually sounds, under `swing`.
///
/// One number doing one thing: the off-beat eighth sits `swing` of the way
/// from where it was written to the following beat. `0.0` is straight, `0.33`
/// is roughly the triplet feel, `0.5` is dotted.
///
/// The beat is **stretched around** that point rather than the eighth being
/// picked out and moved — everything in the first half of the beat is pushed
/// late in proportion, everything in the second half is pulled back in
/// proportion. Two things follow, and both are why it is written this way.
/// Nothing has to decide whether a note *is* an eighth, a question only an
/// epsilon could answer and one that a note written at `0.4999` would answer
/// wrong. And the order of the notes is preserved: a sixteenth written after
/// the off-beat eighth is still after it, where moving the eighth alone would
/// at `0.5` land the two on the same sample.
///
/// It is still *eighth-note* swing: the grid being warped is the eighth, and
/// finer subdivisions ride along with it rather than being stranded against a
/// beat that moved without them. Sixteenth-note swing is the obvious extension
/// and is deliberately not here — one number that does the common thing is
/// worth more than a taxonomy nobody reaches for. A track may swing by its own
/// number ([`Played`]); it is still this one warp.
///
/// Measured **within the pattern**, from `start`, so a pattern swings the same
/// wherever in the arrangement it is played — a pattern is a block of music,
/// not a thing that means something different depending on where it sits.
///
/// Onsets only. A note's `dur` is what the document says it is; a gate that
/// stretched with the beat would change how long a note is held for a reason
/// the document never states.
pub(crate) fn swung(start: f32, swing: f32) -> f32 {
    if swing <= 0.0 {
        return start;
    }
    let beat = start.floor();
    let phase = start - beat;
    // Where the off-beat eighth now sits: half a beat, plus `swing` of the
    // half-beat that remains between it and the next downbeat.
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

    /// The property that makes a warp better than picking the eighth out: two
    /// notes never swap places, and never collide.
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

    /// A `velocity` amount past 1 must not be able to silence or invert a note.
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

    /// The offset is a fraction of the velocity the note is played at, so a
    /// note struck softly strays less in absolute terms than a loud one — and
    /// neither can stray further than the amount asked for.
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
        assert!(nothing.is_nothing());
        assert_eq!(nothing.onset_seconds(0, 7, 1), 0.0);
        assert_eq!(nothing.velocity(0.3, 0, 7, 1), 0.3);
        assert_eq!(nothing.timbre(0.9, 0, 7, 1), 0.0);
    }

    /// One field asked for is not all three drawn: a song that scatters only
    /// tone must land its notes where they were written, at the level written.
    #[test]
    fn the_three_axes_are_drawn_apart() {
        let tone_only = Humanize {
            timbre: 0.2,
            ..Humanize::default()
        };
        assert!(!tone_only.is_nothing());
        assert_eq!(tone_only.onset_seconds(0, 7, 1), 0.0);
        assert_eq!(tone_only.velocity(0.3, 0, 7, 1), 0.3);
        assert_ne!(tone_only.timbre(1.0, 0, 7, 1), 0.0);
    }
}
