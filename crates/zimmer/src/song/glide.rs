//! Which note a glide slides from.

use rusty_core::collections::Map;

use super::{Key, Layer, Note};
use crate::error::SynthError;

pub(crate) struct Slides {
    midi: Vec<f32>,
    previous: Vec<Option<usize>>,
    last: Vec<Option<usize>>,
}

impl Slides {
    pub(crate) fn of(
        notes: &[Note],
        track_of: &Map<&str, usize>,
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
            line.sort_by(|a, b| {
                notes[*a]
                    .start
                    .total_cmp(&notes[*b].start)
                    .then(midi[*a].total_cmp(&midi[*b]))
            });
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

pub(crate) struct Trail(Vec<Option<f32>>);

impl Trail {
    pub(crate) fn new(tracks: usize) -> Self {
        Self(vec![None; tracks])
    }

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

    fn slides(notes: &[Note]) -> Slides {
        let track_of = Map::from_iter([("bass", 0), ("keys", 1)]);
        Slides::of(notes, &track_of, 2).expect("the fixture's notes are named pitches")
    }

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

    #[test]
    fn the_previous_note_is_the_one_that_started_first_not_the_one_written_first() {
        let written = [note("bass", "E2", 1.0), note("bass", "C2", 0.0)];
        assert_eq!(from(&written), vec![Some(36.0), None]);
    }

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

    #[test]
    fn a_hand_is_never_on_another_tracks_note() {
        let both = [
            note("bass", "E2", 0.0),
            note("keys", "C5", 0.5),
            note("bass", "G2", 1.0),
        ];
        assert_eq!(from(&both), vec![None, None, Some(40.0)]);
    }

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
