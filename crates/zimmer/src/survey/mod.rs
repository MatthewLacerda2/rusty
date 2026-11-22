//! What a set of recipes is made of, counted.

mod duty;
mod harmony;
pub(crate) mod pitch;
mod song;
mod walk;

use std::collections::BTreeMap;

pub use harmony::{Clash, Harmony, OutOfKey, Place, Stray, Voice, PASSING};
pub use pitch::{PitchClasses, Register};

#[derive(Debug, Clone, PartialEq)]
pub struct TrackSurvey {
    pub name: String,
    pub source: Option<&'static str>,
    pub gain: f32,
    pub cutoff: Option<f32>,
    /// The amp envelope's sustain, not the source's: a `karplus` or `fm2` patch reading 0.4 can
    /// still decay.
    pub sustain: Option<f32>,
    pub notes: usize,
    pub median: Option<f32>,
    pub duty: f32,
}

impl TrackSurvey {
    pub fn density(&self, seconds: f32) -> Option<f32> {
        (seconds > 0.0).then(|| self.notes as f32 / seconds)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SongSurvey {
    pub name: String,
    pub bpm: f32,
    pub tracks: Vec<TrackSurvey>,
    pub seconds: f32,
    pub register: Option<Register>,
    pub pitches: PitchClasses,
}

impl SongSurvey {
    /// By `gain × duty`, not gain: percussion is written loud because it is short. Ties go to the
    /// first listed.
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

fn room(track: &TrackSurvey) -> f32 {
    track.gain * track.duty
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Survey {
    pub songs: Vec<SongSurvey>,
}

impl Survey {
    pub fn rollup(&self) -> Rollup {
        let mut sources: BTreeMap<&'static str, (usize, usize)> = BTreeMap::new();
        let (mut tempo, mut register) = (None::<(f32, f32)>, None::<Register>);
        for song in &self.songs {
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
        // Most-used first, so the row a reader is looking for is the top one; name breaks a tie, so
        // the order is the same on every run.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceCount {
    pub source: &'static str,
    pub songs: usize,
    pub loudest: usize,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Rollup {
    pub songs: usize,
    pub sources: Vec<SourceCount>,
    pub tempo: Option<(f32, f32)>,
    pub register: Option<Register>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(name: &str, source: &'static str, gain: f32) -> TrackSurvey {
        held(name, source, gain, 1.0)
    }

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
        assert!(cue.kinds().contains("noise"));
    }

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

    #[test]
    fn a_density_over_no_time_is_unanswerable_rather_than_zero() {
        let played = track("arp", "karplus", 0.5);
        assert_eq!(played.density(4.0), Some(2.0));
        assert_eq!(played.density(0.0), None);
        let silent = TrackSurvey { notes: 0, ..played };
        assert_eq!(silent.density(4.0), Some(0.0));
    }

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
