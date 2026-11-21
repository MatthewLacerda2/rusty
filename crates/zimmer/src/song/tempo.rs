//! A song whose tempo moves: the changes written after `bpm`.
//!
//! `bpm` is the tempo the piece **starts** at, and for most songs it is the
//! only one. A piece that speeds up or winds down writes the rest here, one
//! [`TempoChange`] per point, in beats — which keeps every other time in the
//! document in the unit it was always in. Without this, a *più vivo* at beat
//! 100 had to be written by rescaling every note after it by the ratio of the
//! two tempos, and the document stopped saying what the music is: odd
//! fractional beats, step strings that no longer line up with the grid, and
//! "change the tempo" no longer one number.
//!
//! ```jsonc
//! "bpm": 138,
//! "tempo": [
//!   { "beat": 100, "bpm": 154 },               // jumps to 154 on beat 100
//!   { "beat": 132, "bpm": 176, "ramp": true }  // gets there gradually
//! ]
//! ```
//!
//! **A point says what the tempo is on its beat; `ramp` says how it got
//! there.** Without it the tempo holds until the point and then jumps, which
//! is a change of section. With it the tempo moves evenly from the point
//! before — or from `bpm`, for the first — and arrives on the beat written,
//! which is an *accelerando* or a *ritardando*. The flag sits on the point
//! being arrived at rather than the one being left because that is how the
//! music is spoken: "slowing to 60 by the last bar" is one sentence about one
//! place.
//!
//! **"Evenly" is per beat**, not per second: halfway through a ramp in beats is
//! halfway between the two tempos. That is the reading that keeps the
//! document in beats, and the one an author can check by counting.
//!
//! Plain data, and deliberately easy to build from outside: a list of points
//! is what a standard MIDI file's tempo events already are, and a converter
//! writes one of these per event with `ramp` left `false`. The arithmetic
//! that turns the list into seconds is [`super::clock`]'s, and nothing else's.

use serde::{Deserialize, Serialize};

use crate::error::SynthError;

/// One point of a song's tempo map: from this beat on, this tempo.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TempoChange {
    /// Where the change lands, in beats from the start of the arrangement —
    /// after beat 0, since the tempo there is the song's own `bpm`.
    pub beat: f32,
    /// The tempo on that beat, in beats per minute.
    pub bpm: f32,
    /// Whether the tempo moves evenly from the previous point to this one
    /// rather than holding and then jumping. Absent is a jump, which is also
    /// what every tempo change in a MIDI file is.
    #[serde(default, skip_serializing_if = "is_false")]
    pub ramp: bool,
}

impl TempoChange {
    /// A jump to `bpm` on `beat` — the ordinary tempo change, and the only
    /// kind a MIDI file carries.
    pub const fn jump(beat: f32, bpm: f32) -> Self {
        Self {
            beat,
            bpm,
            ramp: false,
        }
    }

    /// A tempo that arrives at `bpm` on `beat`, moving evenly from the point
    /// before.
    pub const fn ramp(beat: f32, bpm: f32) -> Self {
        Self {
            beat,
            bpm,
            ramp: true,
        }
    }
}

/// Refuses a map the clock cannot read: a change on or before the first beat,
/// whose tempo is `bpm`'s to say; a tempo that is not a positive number; and
/// beats that do not ascend, which would have the piece play a stretch of
/// itself twice or not at all.
///
/// Where the last change falls against the arrangement is deliberately not
/// checked, the way an automation point past the end is not: a map is
/// allowed to outlast a piece that was cut shorter than it.
pub(super) fn check(changes: &[TempoChange]) -> Result<(), SynthError> {
    let mut previous = 0.0;
    for (index, change) in changes.iter().enumerate() {
        let TempoChange { beat, bpm, .. } = *change;
        if !(beat.is_finite() && beat > 0.0 && bpm.is_finite() && bpm > 0.0) {
            return Err(SynthError::BadTempoChange { index, beat, bpm });
        }
        if beat <= previous {
            return Err(SynthError::TempoOutOfOrder {
                index,
                beat,
                previous,
            });
        }
        previous = beat;
    }
    Ok(())
}

/// The test that keeps `"ramp": false` out of every saved document — a jump is
/// the plain case, and writing the default into every point would change the
/// bytes a bake is addressed by for nothing.
fn is_false(ramp: &bool) -> bool {
    !*ramp
}
