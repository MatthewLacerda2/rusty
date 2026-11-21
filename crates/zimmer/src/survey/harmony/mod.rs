//! Which written notes sound together, and how far apart they are.
//!
//! The one piece of arithmetic a song document leaves undone. Every note is
//! stated, but pitch reaches a note through routes that compose — names,
//! degrees, chords and their voicings, arpeggios, both transposes, layered
//! entries — and which of those notes overlap in time is not written anywhere.
//! An author who cannot hear works it out by reading JSON, and that is exactly
//! where a bass on F under a pad holding E gets written.
//!
//! ## It reads, and infers nothing
//!
//! **No key is guessed, no chord is named, no progression is judged.** A clash
//! is two notes a semitone apart, on two tracks, held at once; out of key is a
//! note outside the scale the song itself declares. Both are facts about the
//! page. Neither is a refusal or a gate: a rub can be the point, and this says
//! where one is and how long it lasts, so an author who meant it can leave it.
//!
//! ## Time, not counts
//!
//! A chromatic sixteenth passing over a chord is music; a bar-long rub is a
//! mistake. So everything here is weighed in beats, the longest come first,
//! and anything held for less than [`PASSING`] is not counted at all.

use std::fmt;

use crate::note::midi_to_name;
use crate::song::{PatchResolver, Song};
use read::{clashes, pitched, strays};

mod read;
#[cfg(test)]
mod tests;

/// Shorter than this, in beats, and a rub or an accidental is a passing note:
/// an eighth of a beat, a thirty-second note in common time.
pub const PASSING: f32 = 0.125;

/// How many of the longest clashes a report lists.
pub(super) const CLASHES_LISTED: usize = 5;

/// How many of each track's longest accidentals a report lists.
pub(super) const STRAYS_LISTED: usize = 3;

/// One note of a clash: the track playing it and the pitch it sounds at.
#[derive(Debug, Clone, PartialEq)]
pub struct Voice {
    /// The track's name in the song.
    pub track: String,
    /// The pitch it sounds at, in MIDI, after every transpose.
    pub midi: f32,
}

impl fmt::Display for Voice {
    /// `bass F2`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.track, midi_to_name(self.midi))
    }
}

/// Where in the piece something happens: which section, and how far into it.
#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    /// The arrangement entry's label — its pattern, or its layers' joined with
    /// ` + ` — which is how every other report names a section.
    pub section: String,
    /// Beats from the top of that section, counted the way a pattern's
    /// `start` is: the first beat is `0`.
    pub beat: f32,
    /// How many times the arrangement plays it there: a chorus played three
    /// times with the same rub in it is one finding, three times over.
    pub times: usize,
}

impl fmt::Display for Place {
    /// `chorus beat 8`, and `chorus beat 8, 3 times` for a repeat.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} beat {}", self.section, number(self.beat))?;
        if self.times > 1 {
            write!(f, ", {} times", self.times)?;
        }
        Ok(())
    }
}

/// Two notes on two tracks a semitone apart — a minor second, major seventh or
/// minor ninth in any octave — held at the same time.
#[derive(Debug, Clone, PartialEq)]
pub struct Clash {
    /// The note on the track the song lists first.
    pub low: Voice,
    /// The note on the track it lists second.
    pub high: Voice,
    /// How long the two overlap, in beats, each time it happens.
    pub beats: f32,
    /// Where the overlap starts.
    pub at: Place,
}

impl fmt::Display for Clash {
    /// `2 beats: bass F2 against pad E3, chorus beat 8`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {} against {}, {}",
            beats(self.beats),
            self.low,
            self.high,
            self.at
        )
    }
}

/// A note outside the song's key.
#[derive(Debug, Clone, PartialEq)]
pub struct Stray {
    /// The pitch it sounds at, in MIDI.
    pub midi: f32,
    /// How long it is held, in beats.
    pub beats: f32,
    /// Where it starts.
    pub at: Place,
}

impl fmt::Display for Stray {
    /// `F4 for 2 beats, verse beat 3`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} for {}, {}",
            midi_to_name(self.midi),
            beats(self.beats),
            self.at
        )
    }
}

/// One track's notes outside the key.
#[derive(Debug, Clone, PartialEq)]
pub struct OutOfKey {
    /// The track's name in the song.
    pub track: String,
    /// The share of its notes' held time that is outside the key,
    /// `0.0..=1.0`. Held time is summed per note, so a chord of three counts
    /// three times — it is three notes.
    pub share: f32,
    /// Its longest notes outside the key, longest first.
    pub longest: Vec<Stray>,
}

/// What a song's written notes do to each other, and to its key.
///
/// [`Harmony::of`] reads it; its `Display` is the report `synth_check`
/// prints, and [`Harmony::headline`] the one line a bake adds.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Harmony {
    /// The key the song declares, as it spells it. Absent when it declares
    /// none — and then [`Harmony::out_of_key`] is empty, because no key is
    /// guessed for it.
    pub key: Option<String>,
    /// How many clashes there are across one pass of the arrangement, every
    /// repeat counted.
    pub clashes: usize,
    /// How long they last in all, in beats.
    pub clash_beats: f32,
    /// The longest of them, longest first, a repeated one listed once.
    pub longest: Vec<Clash>,
    /// Each pitched track with a note outside the key, in the song's track
    /// order.
    pub out_of_key: Vec<OutOfKey>,
}

impl Harmony {
    /// Reads which of `song`'s notes clash, and which leave its key.
    ///
    /// A track is read when it is **pitched**: its source is not `noise`, and
    /// at least one of its notes states a pitch — a step string playing its
    /// default note is how percussion is written, and states none. A tuned
    /// drum that writes its notes is read, and named in what it is found in,
    /// so a reader can discount it. A patch `resolve` cannot find is read as
    /// pitched: a report would rather say too much about one unreadable
    /// instrument than nothing.
    pub fn of(song: &Song, resolve: &dyn PatchResolver) -> Self {
        let notes = pitched(song, resolve);
        let key = song.key().ok().flatten();
        let (clashes, clash_beats, longest) = clashes(song, &notes);
        Self {
            key: key.and(song.key.clone()),
            clashes,
            clash_beats,
            longest,
            out_of_key: key.map_or_else(Vec::new, |key| {
                song.tracks
                    .iter()
                    .enumerate()
                    .filter_map(|(track, written)| {
                        strays(song, &notes, track, |midi| key.contains(midi)).map(
                            |(share, longest)| OutOfKey {
                                track: written.name.clone(),
                                share,
                                longest,
                            },
                        )
                    })
                    .collect()
            }),
        }
    }

    /// Whether there is nothing to report: no clash, and no note outside a
    /// declared key.
    pub fn is_clean(&self) -> bool {
        self.clashes == 0 && self.out_of_key.is_empty()
    }

    /// One line for a bake's report, or nothing when the song is clean — so a
    /// clean song's report does not grow.
    ///
    /// `2 clashes, longest 2 beats: bass F2 against pad E3, chorus beat 8`
    pub fn headline(&self) -> Option<String> {
        let mut parts = Vec::new();
        if let Some(longest) = self.longest.first() {
            parts.push(format!(
                "{}, longest {longest}",
                count(self.clashes, "clash", "clashes")
            ));
        }
        if !self.out_of_key.is_empty() {
            let key = self.key.as_deref().unwrap_or_default();
            let tracks: Vec<String> = self
                .out_of_key
                .iter()
                .map(|track| format!("{} {}", track.track, percent(track.share)))
                .collect();
            parts.push(format!("out of {key}: {}", tracks.join(", ")));
        }
        (!parts.is_empty()).then(|| format!("harmony: {}", parts.join("; ")))
    }
}

impl fmt::Display for Harmony {
    /// The whole report: every listed clash, then the key section when the
    /// song declares a key.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.clashes == 0 {
            write!(f, "clashes: none")?;
        } else {
            write!(
                f,
                "clashes: {}, {} in all",
                self.clashes,
                beats(self.clash_beats)
            )?;
            for clash in &self.longest {
                write!(f, "\n  {clash}")?;
            }
        }
        let Some(key) = &self.key else {
            return Ok(());
        };
        if self.out_of_key.is_empty() {
            return write!(f, "\nout of {key}: none");
        }
        write!(f, "\nout of {key}:")?;
        for track in &self.out_of_key {
            write!(
                f,
                "\n  {} {} of its held notes",
                track.track,
                percent(track.share)
            )?;
            for stray in &track.longest {
                write!(f, "\n    {stray}")?;
            }
        }
        Ok(())
    }
}

/// `1 clash`, `3 clashes`.
fn count(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// `1 beat`, `2 beats`, `0.25 beats`.
fn beats(n: f32) -> String {
    let unit = if (n - 1.0).abs() < 1e-4 {
        "beat"
    } else {
        "beats"
    };
    format!("{} {unit}", number(n))
}

/// A beat count with no trailing zeros: `8`, `1.5`, `0.25`.
fn number(n: f32) -> String {
    let text = format!("{n:.3}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// `12%`, and `<1%` for a share too small to round to one.
fn percent(share: f32) -> String {
    let rounded = (share * 100.0).round();
    if rounded < 1.0 {
        "<1%".to_owned()
    } else {
        format!("{rounded}%")
    }
}
