//! Reading one song document into the facts a survey reports.
//!
//! Nothing is rendered and nothing is measured — every number here is lifted
//! from the page. The one place that takes any thought is which notes count:
//! the arrangement can transpose a pattern and can silence a track for a
//! playing, so the register a piece actually reaches is not the register its
//! patterns were written in.

use std::collections::{BTreeSet, HashMap};

use super::duty::Sounding;
use super::walk::walk;
use super::{PitchClasses, Register, SongSurvey, TrackSurvey};
use crate::patch::Patch;
use crate::song::clock::Clock;
use crate::song::shape::plan;
use crate::song::{PatchRef, PatchResolver, Song};

impl SongSurvey {
    /// Surveys one song, resolving any track that names its instrument rather
    /// than carrying it inline.
    ///
    /// A reference that cannot be resolved leaves that track's source and
    /// cutoff absent instead of failing: a survey is a report over a whole
    /// project, and one unreadable instrument should cost one field rather than
    /// the other five songs.
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
                    // Taken rather than borrowed: each track is visited once,
                    // and sorting for the median needs the notes owned anyway.
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

    /// Every source kind this song uses, each named once.
    pub(super) fn kinds(&self) -> BTreeSet<&'static str> {
        self.tracks
            .iter()
            .filter_map(|track| track.source)
            .collect()
    }
}

/// The patch behind a track, or nothing when its reference does not resolve.
pub(super) fn instrument(reference: &PatchRef, resolve: &dyn PatchResolver) -> Option<Patch> {
    match reference {
        PatchRef::Inline(patch) => Some((**patch).clone()),
        PatchRef::Named(name) => resolve.resolve(name).ok(),
    }
}

/// Which generator makes a patch's tone, in the word the recipe spells it
/// with — the one place a survey reads it, for the per-track column and for
/// the harmony report's skipping of unpitched sources alike.
pub(super) fn source(patch: &Patch) -> &'static str {
    patch.source.kind()
}

/// The register and pitch classes of every note that actually sounds.
///
/// Read off [`walk`], so the arrangement's own transforms are applied and
/// clamped into the MIDI range exactly as the renderer clamps them — a pattern
/// transposed past the top of the keyboard is reported at the pitch it will be
/// played at rather than the one the arithmetic asked for. A note whose name
/// does not parse is skipped: refusing the document is [`Song::validate`]'s
/// job, not a report's.
/// The last two returns are those same notes kept per track — their pitches,
/// and when each one sounds — which is what turns a report about a song into a
/// report about each instrument in it.
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
        // Before the pitch is read: a note whose name does not parse is still
        // a note this track holds the arrangement for, and the renderer is
        // what refuses the document.
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

/// The middle pitch of some notes, or nothing when there are none.
///
/// Sorted here rather than at the call site because a median is the only thing
/// the order is wanted for. An even count takes the upper of the two middles:
/// averaging them could land a quarter-tone from any note in the piece, and a
/// column that names a pitch nobody played is worse than one that rounds.
fn median(mut notes: Vec<f32>) -> Option<f32> {
    notes.sort_by(f32::total_cmp);
    notes.get(notes.len() / 2).copied()
}

/// How long one pass through the arrangement lasts, at the tempo it plays at.
///
/// `plan` may have stretched the tempo to fit a target, and every other number
/// in this report follows the played tempo rather than the written one — a
/// density measured against the written length would be beside the music by
/// exactly the amount the stretch moved it.
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

    /// Every column of a per-song row comes off the page: what plays it, how
    /// loud it is written, and how it is filtered.
    #[test]
    fn a_song_reports_what_its_documents_say_it_is_made_of() {
        let survey = surveyed();
        assert_eq!(survey.bpm, 96.0);
        assert_eq!(survey.tracks[0].source, Some("karplus"));
        assert_eq!(survey.tracks[0].gain, 0.9);
        assert_eq!(survey.tracks[0].cutoff, Some(4600.0));
        // The pad is layered over noise, and is named for its top-level source.
        assert_eq!(survey.tracks[1].source, Some("osc_stack"));
        assert_eq!(survey.tracks[1].cutoff, None, "no filter, no cutoff");
        assert_eq!(
            survey.loudest().map(|track| track.name.as_str()),
            Some("lead")
        );
    }

    /// What a track *does*, which is the half no source kind predicts. The lead
    /// plays four times over eight beats and the pad once, because the second
    /// arrangement entry leaves the pad out — and the pad is held where the
    /// lead is not, which is the difference `karplus` versus `osc_stack` does
    /// not tell you.
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

    /// A layered entry's layers all start on its first beat, so the same
    /// track in two of them overlaps rather than queues — and the next entry
    /// waits for the longest layer. The pad sounds over beats 1–2.5 of the
    /// layered slot (two notes, one stretch) and 5–6 of the groove after it.
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

    /// The median is a pitch that was actually played, never an average
    /// between two of them: the lead's four notes are C4, E4 and those two a
    /// fifth up, so the upper middle is G4 rather than something between.
    #[test]
    fn the_median_is_a_note_somebody_wrote() {
        let survey = surveyed();
        assert_eq!(survey.tracks[0].median, Some(67.0));
        assert_eq!(survey.tracks[1].median, Some(36.0), "one note is its own");
    }

    /// The register is what is *played*: the second entry transposes the lead
    /// up a fifth and leaves the pad out, and both show.
    #[test]
    fn the_register_follows_the_arrangement_rather_than_the_patterns() {
        let survey = surveyed();
        let register = survey.register.expect("notes were played");
        assert_eq!(register.to_string(), "C2-B4");
        // C and E from the lead, C from the pad, and the fifth above each of
        // the lead's two — the pad's transposed C is absent because the second
        // entry does not play it.
        assert_eq!(survey.pitches.to_string(), "C E G B");
    }

    /// A song nobody wrote a note into has no register at all, rather than one
    /// at zero.
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
