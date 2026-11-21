//! A chord played one voice at a time: `"arp": "up"`.
//!
//! A block [`Chord`] is the harmony *stated*. An arpeggio is the same harmony
//! *played* — a plucked ostinato under a chord loop, a piano figure, a guitar
//! part that is a shape rather than a strum. Before this, a figure like that
//! was N hand-written [`Note`]s, and N is the number that decides what gets
//! written: sixteen eighth-notes over four chords is thirty-two objects, and
//! the same figure as one entry per chord is four.
//!
//! That is not a prediction. A piece written on 2026-08-27 needed exactly this
//! and got a **pedal** instead — one pitch repeated through a step string,
//! because a step string was the only cheap melodic notation there was. A
//! listener heard it as "somebody fretted the top string and played it once,
//! over and over", which is precisely what the document said. The notation did
//! not merely cost tokens; it *selected the music*. That is also the second-order
//! failure worth naming: [`steps`](super::super::steps) says in so many words
//! that it is not for melody, and it was used for melody anyway, because a rule
//! is only followed when it is not also the expensive path.
//!
//! ## It orders voices, it never invents them
//!
//! Everything here is a permutation of the notes the chord already expanded to,
//! repeated. No pitch reaches the output that the document did not write —
//! which is the line between notation and a generator, and the reason this is
//! notation. A voicing no name spells is still spelled as pitches, and it
//! arpeggiates the same way.
//!
//! The order is the order the chord voiced in — low to high for a name, by
//! construction, and as written for a spelled list. `up` is that order and
//! `down` is its reverse; neither sorts anything. A spelled chord written out
//! of order arpeggiates in the order it was written, because reordering a
//! document's pitches behind its back is the one thing worse than playing them
//! as they are.
//!
//! ## Three words, and no fourth
//!
//! `up`, `down`, `up_down` — the same closed-vocabulary judgement
//! [`steps`](super::super::steps) makes about its three characters, for the
//! same reason. A written index sequence (`"0 2 1 2"`) is a second way to write
//! a note list, and it can name a voice the chord does not have. `random` is
//! not readable at all — a figure nobody can see without running it is what a
//! recipe exists to avoid, and it is the objection that kept Euclidean rhythms
//! out. `as_played` would be a synonym for `up`, since the voices arrive in the
//! order the page wrote them.
//!
//! `up_down` turns without repeating either end: four voices are
//! `1 2 3 4 3 2`, then round again. The inclusive reading (`1 2 3 4 4 3 2 1`)
//! doubles the turning notes, which is audible as a stutter and is not what an
//! arpeggiator has meant by the word for forty years.
//!
//! ## What fills the slot is `dur`, not a fourth word
//!
//! The figure repeats until the chord is over, and truncates wherever that
//! lands — a triad in eighths over four beats is `1 2 3 1 2 3 1 2`. So an
//! **ostinato** is a chord as long as the harmony, and a **strum** is a chord
//! exactly one traversal long: `Dm7` at `div` 0.5 with `dur` 2 plays its four
//! voices once and stops. Both readings come out of the field that was already
//! there, which is why there is no `once` beside the three words above.
//!
//! Truncation is deliberate rather than tolerated. Refusing a figure that does
//! not divide its slot — the rule [`steps`](super::super::steps) applies to its
//! string — would be wrong here, because there is no written count to check the
//! grid against. The step string states its length twice on purpose and the
//! redundancy catches a typo; an arpeggio states it once, so a mismatch is not
//! evidence of anything and refusing it would only force `div` gymnastics
//! around a three-note chord.
//!
//! ## Each note is a note from there on
//!
//! Expansion happens before the performance, exactly as a
//! [chord's](super::super::chord) and a [step string's](super::super::steps)
//! does, so every note of the figure takes its own ordinal and therefore its
//! own render seed, its own swing displacement and its own humanise nudge. An
//! arpeggio is a way of *writing* notes and not a second thing for the renderer
//! to know about — which is why it inherits all three for free rather than
//! having to be taught them.

use serde::{Deserialize, Serialize};

use super::super::Note;
use super::super::steps::GRID_EPSILON;
use super::Chord;
use crate::error::SynthError;

/// The most notes one arpeggiated chord may sound.
///
/// A bound rather than a taste: `dur / div` is a count this module allocates,
/// so a mistyped `div` of `0.0001` is otherwise a renderer that never returns.
/// Four thousand notes is five hundred bars of sixteenths under a single chord,
/// which is past any figure and well into a texture — and a texture that long
/// is a pattern, not an entry.
const MAX_FIGURE: usize = 4096;

/// The order an arpeggio walks its chord's voices in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Arp {
    /// Lowest voice to highest, then round again.
    Up,
    /// Highest voice to lowest, then round again.
    Down,
    /// Up and back down, playing neither end twice: four voices are
    /// `1 2 3 4 3 2`.
    UpDown,
}

impl Arp {
    /// One traversal of `voices`, in this order — the figure that then repeats.
    ///
    /// Borrows rather than indexes, so a figure cannot name a voice that is not
    /// there; and an empty chord yields an empty figure instead of a panic,
    /// though [`Chord::voice_into`](super::Chord::voice_into) refuses one
    /// before this is reached.
    fn figure(self, voices: &[Note]) -> Vec<&Note> {
        match self {
            Self::Up => voices.iter().collect(),
            Self::Down => voices.iter().rev().collect(),
            Self::UpDown => {
                let mut figure: Vec<&Note> = voices.iter().collect();
                // The turn: everything but the two ends, coming back. One voice
                // and two voices both leave this empty, which is right — there
                // is nothing between the ends to play on the way down.
                let inner = voices.len().saturating_sub(1);
                figure.extend(voices.iter().take(inner).skip(1).rev());
                figure
            }
        }
    }
}

/// Appends the notes an arpeggiated chord sounds, in time order, or says why it
/// cannot.
///
/// `voices` is what the chord expanded to — the pitches, all sharing its
/// timing — and every note out of here is one of those at its own onset.
/// Nothing is appended when the entry is refused.
pub(super) fn spread(
    chord: &Chord,
    arp: Arp,
    voices: &[Note],
    out: &mut Vec<Note>,
) -> Result<(), SynthError> {
    let refuse = |why| {
        Err(SynthError::BadArp {
            track: chord.track.clone(),
            start: chord.start,
            why,
        })
    };
    let Some(div) = chord.div else {
        return refuse("an `arp` needs a `div` — how long one step of the figure is, in beats");
    };
    if !(div.is_finite() && div > 0.0) {
        return refuse(
            "`div` is how long one step of the figure lasts, so it is a positive number of beats",
        );
    }
    let gate = chord.gate.unwrap_or(div);
    if !(gate.is_finite() && gate > 0.0) {
        return refuse(
            "`gate` is how long one note of the figure sounds, so it is a positive number of beats",
        );
    }
    // A step lands whenever it starts before the chord is over, so the count is
    // a floor — and the epsilon is there for the same reason a step string's
    // grid check carries one: a triplet `div` does not divide a beat exactly.
    let steps = (chord.dur / div + GRID_EPSILON).floor();
    if !steps.is_finite() || steps < 1.0 {
        return refuse(
            "`div` is longer than the chord's `dur`, so the figure has no room to sound a single note",
        );
    }
    if steps > MAX_FIGURE as f32 {
        return refuse(
            "the figure is more than 4096 notes — that is a texture rather than an arpeggio, so raise `div` or shorten the chord",
        );
    }
    let figure = arp.figure(voices);
    out.extend(
        figure
            .iter()
            .cycle()
            .take(steps as usize)
            .enumerate()
            .map(|(step, voice)| Note {
                start: chord.start + step as f32 * div,
                dur: gate,
                ..(*voice).clone()
            }),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::Pitch;
    use super::*;

    /// Voices standing in for a chord's, distinguishable by pitch.
    fn voices(count: usize) -> Vec<Note> {
        (0..count)
            .map(|index| Note {
                track: "keys".to_owned(),
                note: Pitch::Midi(index as f32),
                start: 0.0,
                dur: 4.0,
                vel: 0.8,
                articulation: None,
            })
            .collect()
    }

    /// The figure each word names, as the indices it walks.
    fn walked(arp: Arp, count: usize) -> Vec<f32> {
        arp.figure(&voices(count))
            .iter()
            .map(|note| note.note.to_midi().expect("a MIDI pitch"))
            .collect()
    }

    /// The three words, stated as the order they play — which is what the
    /// module doc promises a reader can predict without running anything.
    #[test]
    fn each_word_walks_the_voices_it_names() {
        assert_eq!(walked(Arp::Up, 4), vec![0.0, 1.0, 2.0, 3.0]);
        assert_eq!(walked(Arp::Down, 4), vec![3.0, 2.0, 1.0, 0.0]);
        assert_eq!(walked(Arp::UpDown, 4), vec![0.0, 1.0, 2.0, 3.0, 2.0, 1.0]);
        assert_eq!(walked(Arp::UpDown, 3), vec![0.0, 1.0, 2.0, 1.0]);
        // Two voices have nothing between their ends, so the turn is empty and
        // `up_down` is `up` — surprising only if you expected a doubled end.
        assert_eq!(walked(Arp::UpDown, 2), walked(Arp::Up, 2));
        assert_eq!(walked(Arp::UpDown, 1), vec![0.0]);
        assert!(walked(Arp::UpDown, 0).is_empty());
    }

    /// The cap the module doc names, kept in step with the sentence a refusal
    /// prints.
    #[test]
    fn the_figure_cap_is_the_one_the_refusal_states() {
        assert_eq!(MAX_FIGURE, 4096, "the refusal says 4096 out loud");
    }
}
