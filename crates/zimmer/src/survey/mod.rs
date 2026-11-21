//! What a set of recipes is made of, counted.
//!
//! Every other measurement in this crate looks at **one** thing: a bake's
//! level, a section of it, a track inside it. Nothing looks at a *set*, and a
//! set is where a whole class of finding lives — six cues can each come out
//! measuring perfectly and still be one instrument played six times, which is
//! the first thing a listener notices and the one thing no per-bake number can
//! see.
//!
//! It is also the cheapest signal there is. All of it is already written down
//! in the documents: a source kind is a word in a patch, a tempo is a number in
//! a song, a register is the notes themselves. No bake, no samples, no ffmpeg
//! — this is arithmetic over JSON that was parsed anyway.
//!
//! ## It counts, and stops
//!
//! **No score, no grade, no recommendation, and no diversity number.** This
//! reports what the documents say and never what to do about it. Six variations
//! on one instrument is a legitimate thing to write on purpose, and a report
//! that tutted at it would be enforcing a taste as a build step — the same rule
//! [`crate::level`] already holds: a metric treated as an ear produces music
//! that optimises the number and gets worse. A diversity score is precisely the
//! metric that would get optimised, so there is not one.
//!
//! What that leaves is worth having on its own: an assistant that cannot hear
//! *can* count, and "karplus is the loudest track in all six" is a sentence it
//! can produce from the files without anyone listening first.
//!
//! [`Harmony`] is the one report here about **one** song, and it holds the
//! same line: it reads which written notes sound together and how far apart
//! they are, against the key the song declares, and infers no key, chord or
//! progression of its own.

mod duty;
mod harmony;
pub(crate) mod pitch;
mod song;
mod walk;

use std::collections::BTreeMap;

pub use harmony::{Clash, Harmony, OutOfKey, PASSING, Place, Stray, Voice};
pub use pitch::{PitchClasses, Register};

/// One instrument of one song, as its documents state it.
///
/// Everything here is written down rather than measured: the gain is the
/// number in the score, not the level the track came out at. What it came out
/// at is [`crate::level::Layer`], and that costs a bake.
///
/// Two halves, and the split is the useful part. [`TrackSurvey::source`],
/// `gain` and `cutoff` say what an instrument **is**; `sustain`, `notes`,
/// `median` and `duty` say what it **does** in the piece — whether it is held
/// or struck, how often it plays, where it sits, and how much of the piece it
/// occupies. A patch answers the first three and the arrangement answers the
/// rest, which is how three different source kinds can read as three
/// instruments on the left and as one on the right.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackSurvey {
    /// The track's name in the song.
    pub name: String,
    /// Which generator makes its tone, in the word the recipe spells it with.
    /// Absent when the patch it names could not be resolved — a survey reports
    /// the documents it could read rather than refusing over one it could not.
    pub source: Option<&'static str>,
    /// Its linear gain in the mix, as written.
    pub gain: f32,
    /// The filter's base cutoff in Hz, absent when the patch has no filter.
    /// The envelope and velocity move it from there; this is where it starts.
    pub cutoff: Option<f32>,
    /// The amp envelope's sustain level, absent for the same unresolved patch
    /// that leaves `source` absent.
    ///
    /// **The envelope's sustain, not the source's**, and the difference is
    /// real: a `karplus` string damps on its own and an `fm2` modulator decays
    /// on its own, so a patch can read `0.4` here and still be a decaying
    /// sound. What this states is what the document states — the same
    /// written-not-measured discipline `gain` follows.
    pub sustain: Option<f32>,
    /// How many notes it plays across the whole arrangement, counted after the
    /// arrangement's own `tracks` filters have had their say.
    pub notes: usize,
    /// The median pitch of those notes, in MIDI. Absent when it plays none.
    ///
    /// The median rather than a mean, so one octave leap does not move where a
    /// track is reported to sit. With an even number of notes it is the upper
    /// of the two middle pitches rather than their average, which could land
    /// between two semitones and name a note nobody wrote.
    pub median: Option<f32>,
    /// The share of the arrangement this track is sounding over, `0.0..=1.0`.
    ///
    /// The **union** of its notes rather than the sum, so a chord held for a
    /// bar is one bar of sound. Notes ringing past the last beat are cut off
    /// there, since a song's length is where its final pattern ends.
    ///
    /// This is what stops `gain` from being read as prominence. Percussion is
    /// written loud *because* it is short, so a hat at `0.60` for a fifth of
    /// each beat and a pad at `0.50` held throughout are nowhere near each
    /// other — see [`SongSurvey::loudest`].
    pub duty: f32,
}

impl TrackSurvey {
    /// How often it plays, in notes per second of the piece.
    ///
    /// Absent when the song has no length to divide by — a density over no
    /// time is unanswerable rather than zero, and the two must not print the
    /// same way.
    pub fn density(&self, seconds: f32) -> Option<f32> {
        (seconds > 0.0).then(|| self.notes as f32 / seconds)
    }
}

/// One song: its tempo, its instruments, and the pitch material it is made of.
#[derive(Debug, Clone, PartialEq)]
pub struct SongSurvey {
    /// What to call it in a report — the asset's id, when a project supplied it.
    pub name: String,
    /// The tempo it *plays* at, which is the written one unless `fit` stretched
    /// it. Reporting the written tempo of a stretched song would put every
    /// number in the rollup slightly beside the music.
    pub bpm: f32,
    /// Its instruments, in the order the document lists them.
    pub tracks: Vec<TrackSurvey>,
    /// How long one pass through the arrangement lasts, in seconds, at the
    /// tempo it plays at. Zero for a song with nothing arranged.
    ///
    /// Here rather than as a note-density on each track, because it is a fact
    /// about the song and dividing is the report's job. One pass rather than
    /// the whole `fit`ted bake: repeating a piece scales the note count and the
    /// length by the same factor, so density is the same number either way and
    /// one pass is the one that needs no explaining.
    pub seconds: f32,
    /// How high and low its notes reach. Absent when nothing is played.
    pub register: Option<Register>,
    /// Which pitch classes those notes fall in.
    pub pitches: PitchClasses,
}

impl SongSurvey {
    /// The track taking up the most room in the piece: `gain × duty`.
    ///
    /// **Not the highest gain**, which is what this used to be and which was
    /// wrong in half the songs it was tried on. Written gain is a *per-note
    /// peak*: it says how loud a note is and nothing about how much of the
    /// piece there is any note at all, and percussion is written loud precisely
    /// because it is short. Ranking by gain alone crowned a hi-hat sitting 22 dB
    /// under the actual lead, and then the rollup built on it reported a set
    /// more varied than the one that existed — which is the one thing that
    /// report is for.
    ///
    /// Multiplying by [`TrackSurvey::duty`] fixes that with arithmetic over
    /// documents this already parses, so the survey still costs no bake, no
    /// ffmpeg and no network. **It is a better proxy and not an ear**: a
    /// plucked harp 27 dB down can still be the instrument you hear, because
    /// transients carry prominence that neither a written gain nor an energy
    /// average knows about. Nothing here names a lead with authority.
    ///
    /// Ties go to the first, which is the order the document lists them in.
    pub fn loudest(&self) -> Option<&TrackSurvey> {
        self.tracks.iter().reduce(|loudest, track| {
            if room(track) > room(loudest) {
                track
            } else {
                loudest
            }
        })
    }
}

/// How much of the mix a track takes up: how loud it is written, over how much
/// of the piece it is written at all.
fn room(track: &TrackSurvey) -> f32 {
    track.gain * track.duty
}

/// Every song a project carries, in the order its assets are listed.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Survey {
    /// One entry per song recipe.
    pub songs: Vec<SongSurvey>,
}

impl Survey {
    /// The same facts, counted across the set.
    pub fn rollup(&self) -> Rollup {
        let mut sources: BTreeMap<&'static str, (usize, usize)> = BTreeMap::new();
        let (mut tempo, mut register) = (None::<(f32, f32)>, None::<Register>);
        for song in &self.songs {
            // Once per song, not once per track: a song with three karplus
            // tracks is one song using karplus, and counting tracks would make
            // a thickly-scored piece look like several pieces.
            for kind in song.kinds() {
                sources.entry(kind).or_default().0 += 1;
            }
            if let Some(kind) = song.loudest().and_then(|track| track.source) {
                sources.entry(kind).or_default().1 += 1;
            }
            tempo = Some(match tempo {
                Some((low, high)) => (low.min(song.bpm), high.max(song.bpm)),
                None => (song.bpm, song.bpm),
            });
            register = match (register, song.register) {
                (Some(spread), Some(one)) => Some(spread.union(one)),
                (spread, one) => spread.or(one),
            };
        }
        let mut sources: Vec<SourceCount> = sources
            .into_iter()
            .map(|(source, (songs, loudest))| SourceCount {
                source,
                songs,
                loudest,
            })
            .collect();
        // Most-used first, so the row a reader is looking for is the top one;
        // name breaks a tie, so the order is the same on every run.
        sources.sort_by(|a, b| {
            b.songs
                .cmp(&a.songs)
                .then(b.loudest.cmp(&a.loudest))
                .then(a.source.cmp(b.source))
        });
        Rollup {
            songs: self.songs.len(),
            sources,
            tempo,
            register,
        }
    }
}

/// How often one source kind turns up across the set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceCount {
    /// The kind, as a recipe spells it.
    pub source: &'static str,
    /// How many songs have at least one track using it.
    pub songs: usize,
    /// How many songs have it in their loudest track, by
    /// [`SongSurvey::loudest`]'s meaning of the word — the count the per-song
    /// rows cannot add up on their own, and the one line in this project that
    /// can catch *six cues, one instrument*.
    ///
    /// Which is why it has to be right: this counted a hi-hat as the sound of
    /// two pieces once, and the result read as a **more varied** set than the
    /// one on disk. A wrong answer here is worse than none, because the whole
    /// point of the line is that a client with no ears takes it literally.
    pub loudest: usize,
}

/// The set, counted.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Rollup {
    /// How many songs were surveyed.
    pub songs: usize,
    /// One entry per source kind used anywhere, most-used first.
    pub sources: Vec<SourceCount>,
    /// The slowest and fastest tempo. Absent when there are no songs.
    pub tempo: Option<(f32, f32)>,
    /// The lowest and highest note anything plays. Absent when nothing does.
    pub register: Option<Register>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(name: &str, source: &'static str, gain: f32) -> TrackSurvey {
        held(name, source, gain, 1.0)
    }

    /// A track written at `gain` and sounding over `duty` of the piece.
    fn held(name: &str, source: &'static str, gain: f32, duty: f32) -> TrackSurvey {
        TrackSurvey {
            name: name.to_owned(),
            source: Some(source),
            gain,
            cutoff: None,
            sustain: Some(0.4),
            notes: 8,
            median: Some(60.0),
            duty,
        }
    }

    /// The bug this report was filed for: a hat written louder than the lead,
    /// firing for a fifth of each beat. Gain alone crowns it; gain over the
    /// room it takes up does not.
    #[test]
    fn a_short_loud_hit_does_not_outrank_what_is_playing_underneath_it() {
        let cue = song(
            "01-km",
            96.0,
            vec![
                held("hat", "noise", 0.60, 0.2),
                held("rhodes", "osc_stack", 0.50, 0.95),
            ],
        );
        assert_eq!(
            cue.loudest().map(|track| track.name.as_str()),
            Some("rhodes")
        );
        // The hat is still *in* the piece and still counted as a source kind.
        // What it has stopped doing is claiming to be the sound of it.
        assert!(cue.kinds().contains("noise"));
    }

    /// A track nobody arranged takes up none of the piece, so it cannot lead
    /// one however loudly it is written.
    #[test]
    fn a_track_that_never_plays_leads_nothing() {
        let cue = song(
            "quiet",
            120.0,
            vec![
                held("unused", "fm2", 1.0, 0.0),
                held("bed", "osc_stack", 0.2, 0.8),
            ],
        );
        assert_eq!(cue.loudest().map(|track| track.name.as_str()), Some("bed"));
    }

    fn song(name: &str, bpm: f32, tracks: Vec<TrackSurvey>) -> SongSurvey {
        SongSurvey {
            name: name.to_owned(),
            bpm,
            tracks,
            seconds: 4.0,
            register: Some(Register::at(40.0)),
            pitches: PitchClasses::default(),
        }
    }

    /// A density needs a length to divide by, and a song with nothing arranged
    /// has none — which is a different answer from "it never plays".
    #[test]
    fn a_density_over_no_time_is_unanswerable_rather_than_zero() {
        let played = track("arp", "karplus", 0.5);
        assert_eq!(played.density(4.0), Some(2.0));
        assert_eq!(played.density(0.0), None);
        let silent = TrackSurvey { notes: 0, ..played };
        assert_eq!(silent.density(4.0), Some(0.0));
    }

    /// The finding the whole report exists for, as a count: one source kind is
    /// the loudest thing in every piece.
    #[test]
    fn the_rollup_counts_which_kind_is_loudest_and_in_how_many() {
        let survey = Survey {
            songs: vec![
                song(
                    "one",
                    96.0,
                    vec![
                        track("arp", "karplus", 0.85),
                        track("pad", "osc_stack", 0.4),
                    ],
                ),
                song(
                    "two",
                    132.0,
                    vec![
                        track("arp", "karplus", 0.76),
                        track("pad", "osc_stack", 0.5),
                    ],
                ),
            ],
        };
        let rollup = survey.rollup();
        assert_eq!(rollup.songs, 2);
        assert_eq!(rollup.sources[0].source, "karplus");
        assert_eq!(rollup.sources[0].songs, 2);
        assert_eq!(rollup.sources[0].loudest, 2);
        assert_eq!(rollup.sources[1].source, "osc_stack");
        assert_eq!(rollup.sources[1].loudest, 0);
        assert_eq!(rollup.tempo, Some((96.0, 132.0)));
    }

    /// A song counts once for a kind however many tracks use it.
    #[test]
    fn one_song_scored_thickly_is_still_one_song() {
        let survey = Survey {
            songs: vec![song(
                "thick",
                100.0,
                vec![
                    track("a", "karplus", 0.5),
                    track("b", "karplus", 0.5),
                    track("c", "karplus", 0.5),
                ],
            )],
        };
        assert_eq!(survey.rollup().sources[0].songs, 1);
    }

    #[test]
    fn nothing_surveyed_counts_nothing() {
        let rollup = Survey::default().rollup();
        assert_eq!(rollup.songs, 0);
        assert!(rollup.sources.is_empty());
        assert_eq!(rollup.tempo, None);
    }
}
