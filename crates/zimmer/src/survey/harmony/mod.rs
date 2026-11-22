//! Which written notes sound together, and how far apart they are.

use std::fmt;

use crate::note::midi_to_name;
use crate::song::{PatchResolver, Song};
use read::{clashes, pitched, strays};

mod read;
#[cfg(test)]
mod tests;

/// In beats, a thirty-second note in 4/4: a rub or accidental shorter than this passes.
pub const PASSING: f32 = 0.125;

pub(super) const CLASHES_LISTED: usize = 5;

pub(super) const STRAYS_LISTED: usize = 3;

#[derive(Debug, Clone, PartialEq)]
pub struct Voice {
    pub track: String,
    pub midi: f32,
}

impl fmt::Display for Voice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.track, midi_to_name(self.midi))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    pub section: String,
    pub beat: f32,
    pub times: usize,
}

impl fmt::Display for Place {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} beat {}", self.section, number(self.beat))?;
        if self.times > 1 {
            write!(f, ", {} times", self.times)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Clash {
    /// The track the song lists first, not the lower pitch.
    pub low: Voice,
    pub high: Voice,
    pub beats: f32,
    pub at: Place,
}

impl fmt::Display for Clash {
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

#[derive(Debug, Clone, PartialEq)]
pub struct Stray {
    pub midi: f32,
    pub beats: f32,
    pub at: Place,
}

impl fmt::Display for Stray {
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

#[derive(Debug, Clone, PartialEq)]
pub struct OutOfKey {
    pub track: String,
    /// Of the track's held time, summed per note, so a chord of three counts three times.
    pub share: f32,
    pub longest: Vec<Stray>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Harmony {
    pub key: Option<String>,
    pub clashes: usize,
    pub clash_beats: f32,
    pub longest: Vec<Clash>,
    pub out_of_key: Vec<OutOfKey>,
}

impl Harmony {
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

    pub fn is_clean(&self) -> bool {
        self.clashes == 0 && self.out_of_key.is_empty()
    }

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

fn count(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn beats(n: f32) -> String {
    let unit = if (n - 1.0).abs() < 1e-4 {
        "beat"
    } else {
        "beats"
    };
    format!("{} {unit}", number(n))
}

fn number(n: f32) -> String {
    let text = format!("{n:.3}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

fn percent(share: f32) -> String {
    let rounded = (share * 100.0).round();
    if rounded < 1.0 {
        "<1%".to_owned()
    } else {
        format!("{rounded}%")
    }
}
