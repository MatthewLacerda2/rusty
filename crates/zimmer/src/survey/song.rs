//! Reading one song document into the facts a survey reports.

use std::collections::{BTreeSet, HashMap};

use super::duty::Sounding;
use super::walk::walk;
use super::{PitchClasses, Register, SongSurvey, TrackSurvey};
use crate::patch::Patch;
use crate::song::clock::Clock;
use crate::song::shape::plan;
use crate::song::{PatchRef, PatchResolver, Song};

impl SongSurvey {
    pub fn of(name: &str, song: &Song, resolve: &dyn PatchResolver) -> Self {
        let (clock, _) = plan(song);
        let (register, pitches, mut by_track, mut sounding) = played(song);
        let beats = song.arrangement_beats();
        Self {
            name: name.to_owned(),
            bpm: clock.bpm(),
            tracks: song
                .tracks
                .iter()
                .map(|track| {
                    let patch = instrument(&track.patch, resolve);
                    let notes = by_track.remove(track.name.as_str()).unwrap_or_default();
                    TrackSurvey {
                        name: track.name.clone(),
                        source: patch.as_ref().map(source),
                        gain: track.gain,
                        cutoff: patch
                            .as_ref()
                            .and_then(|patch| patch.filter)
                            .map(|filter| filter.cutoff),
                        sustain: patch.as_ref().map(|patch| patch.amp.s),
                        notes: notes.len(),
                        median: median(notes),
                        duty: sounding
                            .remove(track.name.as_str())
                            .unwrap_or_default()
                            .over(beats),
                    }
                })
                .collect(),
            seconds: length(song, &clock),
            register,
            pitches,
        }
    }

    pub(super) fn kinds(&self) -> BTreeSet<&'static str> {
        self.tracks
            .iter()
            .filter_map(|track| track.source)
            .collect()
    }
}

pub(super) fn instrument(reference: &PatchRef, resolve: &dyn PatchResolver) -> Option<Patch> {
    match reference {
        PatchRef::Inline(patch) => Some((**patch).clone()),
        PatchRef::Named(name) => resolve.resolve(name).ok(),
    }
}

pub(super) fn source(patch: &Patch) -> &'static str {
    patch.source.kind()
}

type Played = (
    Option<Register>,
    PitchClasses,
    HashMap<String, Vec<f32>>,
    HashMap<String, Sounding>,
);

fn played(song: &Song) -> Played {
    let mut register: Option<Register> = None;
    let mut pitches = PitchClasses::default();
    let mut by_track: HashMap<String, Vec<f32>> = HashMap::new();
    let mut sounding: HashMap<String, Sounding> = HashMap::new();
    walk(song, |note| {
        sounding
            .entry(note.track.clone())
            .or_default()
            .add(note.start, note.dur);
        let Some(midi) = note.midi else {
            return;
        };
        register = Some(register.map_or_else(|| Register::at(midi), |so_far| so_far.with(midi)));
        pitches.add(midi);
        by_track.entry(note.track).or_default().push(midi);
    });
    (register, pitches, by_track, sounding)
}

fn median(mut notes: Vec<f32>) -> Option<f32> {
    notes.sort_by(f32::total_cmp);
    notes.get(notes.len() / 2).copied()
}

fn length(song: &Song, clock: &Clock) -> f32 {
    if clock.bpm() <= 0.0 {
        return 0.0;
    }
    clock.seconds(song.arrangement_beats())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::song::InlineOnly;

    const SONG: &str = r#"{
      "bpm": 96,
      "tracks": [
        { "name": "lead", "gain": 0.9, "patch": {
            "source": { "kind": "karplus" },
            "amp": { "a": 0.01, "d": 0.2, "s": 0.4, "r": 0.3 },
            "filter": { "kind": "lowpass", "cutoff": 4600 } } },
        { "name": "pad", "gain": 0.4, "patch": {
            "source": { "kind": "osc_stack", "oscs": [{ "wave": "saw" }] },
            "amp": { "a": 0.2, "d": 0.1, "s": 0.8, "r": 0.5 },
            "layers": [{ "source": { "kind": "noise" },
                "amp": { "a": 0.2, "d": 0.1, "s": 0.8, "r": 0.5 } }] } }
      ],
      "patterns": { "a": { "beats": 4, "notes": [
        { "track": "lead", "note": "C4", "start": 0, "dur": 1 },
        { "track": "lead", "note": "E4", "start": 1, "dur": 1 },
        { "track": "pad", "note": "C2", "start": 0, "dur": 4 }] } },
      "arrangement": ["a", { "pattern": "a", "transpose": 7, "tracks": ["lead"] }]
    }"#;

    fn surveyed() -> SongSurvey {
        let song = Song::from_json(SONG).expect("the fixture parses");
        SongSurvey::of("theme", &song, &InlineOnly)
    }

    #[test]
    fn a_song_reports_what_its_documents_say_it_is_made_of() {
        let survey = surveyed();
        assert_eq!(survey.bpm, 96.0);
        assert_eq!(survey.tracks[0].source, Some("karplus"));
        assert_eq!(survey.tracks[0].gain, 0.9);
        assert_eq!(survey.tracks[0].cutoff, Some(4600.0));
        assert_eq!(survey.tracks[1].source, Some("osc_stack"));
        assert_eq!(survey.tracks[1].cutoff, None, "no filter, no cutoff");
        assert_eq!(
            survey.loudest().map(|track| track.name.as_str()),
            Some("lead")
        );
    }

    #[test]
    fn a_track_reports_how_it_behaves_and_not_only_what_it_is() {
        let survey = surveyed();
        // Two entries of four beats at 96 bpm.
        assert!((survey.seconds - 5.0).abs() < 0.001, "{}", survey.seconds);

        let lead = &survey.tracks[0];
        assert_eq!(lead.sustain, Some(0.4));
        assert_eq!(lead.notes, 4);
        assert_eq!(lead.density(survey.seconds), Some(0.8));

        let pad = &survey.tracks[1];
        assert_eq!(pad.sustain, Some(0.8));
        assert_eq!(pad.notes, 1);
        assert_eq!(pad.density(survey.seconds), Some(0.2));
    }

    #[test]
    fn layers_share_their_slot_and_the_next_entry_waits_for_the_longest() {
        let song = Song::from_json(
            r#"{ "bpm": 120,
              "tracks": [{ "name": "pad", "patch": {
                  "source": { "kind": "noise" },
                  "amp": { "a": 0.01, "d": 0.1, "s": 1, "r": 0.1 } } }],
              "patterns": {
                "groove": { "beats": 2, "notes": [
                  { "track": "pad", "note": "C3", "start": 1, "dur": 1 }] },
                "solo": { "beats": 4, "notes": [
                  { "track": "pad", "note": "E3", "start": 1.5, "dur": 1 }] } },
              "arrangement": [{ "layers": ["groove", "solo"] }, "groove"] }"#,
        )
        .expect("the fixture parses");
        let survey = SongSurvey::of("layered", &song, &InlineOnly);
        assert_eq!(survey.tracks[0].notes, 3);
        assert!(
            (survey.tracks[0].duty - 2.5 / 6.0).abs() < 1e-6,
            "{}",
            survey.tracks[0].duty
        );
    }

    #[test]
    fn the_median_is_a_note_somebody_wrote() {
        let survey = surveyed();
        assert_eq!(survey.tracks[0].median, Some(67.0));
        assert_eq!(survey.tracks[1].median, Some(36.0), "one note is its own");
    }

    #[test]
    fn the_register_follows_the_arrangement_rather_than_the_patterns() {
        let survey = surveyed();
        let register = survey.register.expect("notes were played");
        assert_eq!(register.to_string(), "C2-B4");
        assert_eq!(survey.pitches.to_string(), "C E G B");
    }

    #[test]
    fn a_song_with_no_notes_has_no_register() {
        let song =
            Song::from_json(r#"{ "bpm": 120, "tracks": [], "patterns": {}, "arrangement": [] }"#)
                .expect("parses");
        let survey = SongSurvey::of("empty", &song, &InlineOnly);
        assert_eq!(survey.register, None);
        assert!(survey.pitches.is_empty());
    }
}
