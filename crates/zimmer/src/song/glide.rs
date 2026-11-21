//! Which note a glide slides from.
//!
//! Every other articulation is a property of the note it is written over.
//! [`Glide`](super::Articulation::Glide) is not: it says *this pitch is
//! arrived at*, and where from is the note before it on the same track. This
//! module is the whole of what "the note before it" means, and
//! [`render`](super::render)'s module doc argues each of the three decisions
//! in it. In short:
//!
//! - **The note that last *started*.** Time, never the order the entries
//!   happen to be written in.
//! - **Ties go to the top.** Notes starting together are one moment: each
//!   slides from whatever preceded the moment, and the line goes on from the
//!   highest of them.
//! - **A note that did not sound was still played.** Muting a track skips its
//!   notes without spending the score.
//!
//! Two types, because the question is asked at two scales. [`Slides`] is one
//! pattern, worked out once however many times the arrangement plays it;
//! [`Trail`] is the walk over the arrangement, holding where each track's hand
//! has got to so a pattern can slide in from the one before it.
//!
//! Everything here is in **written** MIDI, and turns into a played pitch only
//! at the moment it is read — an arrangement entry may transpose, and a hand
//! is where the transposed part put it.

use std::collections::HashMap;

use super::{Key, Layer, Note};
use crate::error::SynthError;

/// Which note each note of a pattern slides from, and where the pattern leaves
/// each track's hand.
///
/// Indices are into the pattern's **voiced** notes — every written form
/// already expanded — in the order the renderer walks them, so a caller pairs
/// one of these with a note by position and never by identity.
pub(crate) struct Slides {
    /// The written MIDI of each note.
    midi: Vec<f32>,
    /// For each note, the note on its track that last started before it.
    previous: Vec<Option<usize>>,
    /// For each track, the note this pattern leaves its hand on.
    last: Vec<Option<usize>>,
}

impl Slides {
    /// Works out both relations for one pattern's voiced `notes`, over a song
    /// of `tracks` tracks that `track_of` names.
    ///
    /// This is where a note's name becomes a number, which is why it can fail:
    /// a pitch is resolved here for every note of the pattern rather than only
    /// for the ones that sound, because a muted note is still where the hand
    /// was.
    pub(crate) fn of(
        notes: &[Note],
        track_of: &HashMap<&str, usize>,
        tracks: usize,
    ) -> Result<Self, SynthError> {
        let midi = notes
            .iter()
            .map(|note| note.note.to_midi())
            .collect::<Result<Vec<f32>, SynthError>>()?;
        let mut lines: Vec<Vec<usize>> = vec![Vec::new(); tracks];
        for (index, note) in notes.iter().enumerate() {
            lines[track_of[note.track.as_str()]].push(index);
        }

        let mut previous = vec![None; notes.len()];
        let mut last = vec![None; tracks];
        for (track, mut line) in lines.into_iter().enumerate() {
            // By onset, and then by pitch so the last of a run of simultaneous
            // notes is the highest of them. `total_cmp` rather than a partial
            // compare because a sort must be total: the starts are validated
            // finite long before this, so the ordering it gives a `NaN` is a
            // formality rather than a case.
            line.sort_by(|a, b| {
                notes[*a]
                    .start
                    .total_cmp(&notes[*b].start)
                    .then(midi[*a].total_cmp(&midi[*b]))
            });
            // Walked as runs of one onset rather than note by note, which is
            // the whole of the tie rule: `ended` is the top of the last run to
            // finish and every note of the run in progress slides from it.
            // `running` is `None` only before the first note of the line, and
            // the first note of a line has no predecessor either, so the two
            // are the same nothing.
            let mut ended = None;
            let mut running: Option<usize> = None;
            let mut at = f32::NEG_INFINITY;
            for index in line {
                if notes[index].start > at {
                    ended = running;
                    at = notes[index].start;
                }
                previous[index] = ended;
                running = Some(index);
            }
            last[track] = running;
        }
        Ok(Self {
            midi,
            previous,
            last,
        })
    }
}

/// Where each track's line has got to: the pitch its hand is on, carried from
/// one arrangement entry to the next.
pub(crate) struct Trail(Vec<Option<f32>>);

impl Trail {
    /// A hand on no track is on any note — which is what makes the first note
    /// of a track play plain rather than slide in from somewhere.
    pub(crate) fn new(tracks: usize) -> Self {
        Self(vec![None; tracks])
    }

    /// The pitch note `index` of `slides` slides from, as `layer` plays it:
    /// the note before it in the pattern, or wherever the last entry left this
    /// `track`'s hand.
    pub(crate) fn from(
        &self,
        slides: &Slides,
        index: usize,
        track: usize,
        layer: &Layer,
        key: Option<&Key>,
    ) -> Option<f32> {
        match slides.previous[index] {
            Some(before) => Some(layer.played_pitch(slides.midi[before], key)),
            None => self.0[track],
        }
    }

    /// Moves every hand this pattern touched to where `layer` left it.
    ///
    /// A track the pattern has no notes on is left where it was, so a bar of
    /// rest is not a hand lifted: the next note on that track still slides
    /// from the last one played on it.
    pub(crate) fn advance(&mut self, slides: &Slides, layer: &Layer, key: Option<&Key>) {
        for (track, last) in slides.last.iter().enumerate() {
            if let Some(index) = last {
                self.0[track] = Some(layer.played_pitch(slides.midi[*index], key));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::song::{Pitch, Play};

    /// A note on `track`, with only the two fields this module reads.
    fn note(track: &str, name: &str, start: f32) -> Note {
        Note {
            track: track.to_owned(),
            note: Pitch::Name(name.to_owned()),
            start,
            dur: 1.0,
            vel: 1.0,
            articulation: None,
        }
    }

    /// The relations for `notes` over a song of two tracks, `bass` and `keys`.
    fn slides(notes: &[Note]) -> Slides {
        let track_of = HashMap::from([("bass", 0), ("keys", 1)]);
        Slides::of(notes, &track_of, 2).expect("the fixture's notes are named pitches")
    }

    /// The pitch each note slides from, played untransposed and in no key.
    fn from(notes: &[Note]) -> Vec<Option<f32>> {
        let slides = slides(notes);
        let entry = Layer::Name("a".to_owned());
        let trail = Trail::new(2);
        (0..notes.len())
            .map(|index| {
                let track = if notes[index].track == "bass" { 0 } else { 1 };
                trail.from(&slides, index, track, &entry, None)
            })
            .collect()
    }

    /// The order the entries are written in decides nothing. Reordering a
    /// `notes` array changes no music today, and a glide must not be the first
    /// thing that makes it.
    #[test]
    fn the_previous_note_is_the_one_that_started_first_not_the_one_written_first() {
        let written = [note("bass", "E2", 1.0), note("bass", "C2", 0.0)];
        assert_eq!(from(&written), vec![Some(36.0), None]);
    }

    /// Notes that start together are one moment: each slides from what
    /// preceded the moment rather than from each other, and the line goes on
    /// from the top of the chord — the **highest** voice of it, which is not
    /// the same claim as the last one written. The chord here is spelled with
    /// its top voice first, so the two answers differ.
    #[test]
    fn simultaneous_notes_share_a_previous_and_the_highest_leads_on() {
        let chord = [
            note("keys", "C3", 0.0),
            note("keys", "A4", 1.0),
            note("keys", "D4", 1.0),
            note("keys", "F4", 1.0),
            note("keys", "B4", 2.0),
        ];
        let root = Some(48.0);
        assert_eq!(from(&chord), vec![None, root, root, root, Some(69.0)]);
    }

    /// One track's notes are nothing to do with another's, however they
    /// interleave in time.
    #[test]
    fn a_hand_is_never_on_another_tracks_note() {
        let both = [
            note("bass", "E2", 0.0),
            note("keys", "C5", 0.5),
            note("bass", "G2", 1.0),
        ];
        assert_eq!(from(&both), vec![None, None, Some(40.0)]);
    }

    /// A pattern picks up where the last one left off, transposed as that one
    /// was played — the hand is where the part put it, not where the page
    /// wrote it.
    #[test]
    fn a_hand_stays_where_the_previous_entry_left_it() {
        let notes = [note("bass", "E2", 0.0), note("bass", "G2", 1.0)];
        let slides = slides(&notes);
        let up = Layer::Transformed(Play {
            transpose: Some(12.0),
            ..play()
        });
        let mut trail = Trail::new(2);
        trail.advance(&slides, &up, None);
        let plain = Layer::Name("a".to_owned());
        assert_eq!(
            trail.from(&slides, 0, 0, &plain, None),
            Some(55.0),
            "the second pass slides from the first pass's G2, an octave up"
        );
    }

    /// A track this pattern is silent on keeps the note it was last on: a bar
    /// of rest is not a hand lifted off the instrument.
    #[test]
    fn a_pattern_that_is_silent_on_a_track_leaves_its_hand_alone() {
        let entry = Layer::Name("a".to_owned());
        let mut trail = Trail::new(2);
        trail.advance(&slides(&[note("bass", "E2", 0.0)]), &entry, None);
        trail.advance(&slides(&[note("keys", "C5", 0.0)]), &entry, None);
        let back_to_the_bass = [note("bass", "A2", 0.0)];
        assert_eq!(
            trail.from(&slides(&back_to_the_bass), 0, 0, &entry, None),
            Some(40.0),
            "the bass hand moved while the bass was resting"
        );
    }

    /// The long form of an entry, with everything about the playing unset.
    fn play() -> Play {
        Play {
            pattern: "a".to_owned(),
            transpose: None,
            transpose_degrees: None,
            vel_scale: None,
            tracks: None,
        }
    }
}
