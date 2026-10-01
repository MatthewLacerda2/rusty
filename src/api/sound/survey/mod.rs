//! src/api/sound/survey/mod.rs — what a **set** of patches and songs is made of,
//! counted (#380): `Sound.Survey(paths)`.
//!
//! Every other `Sound` measurement looks at one bake. This one looks at a set, and
//! catches what no per-bake number can: **the same instrument, everywhere** — six
//! cues that each pass every check and are one guitar six times, or fourteen impact
//! sounds that are all `noise` under a lowpass between 700 and 900 Hz. It reads the
//! documents only: no bake, no decode, no samples.
//!
//! Songs are zimmer's survey (`zimmer::survey`): per track the source, gain,
//! cutoff, sustain, note count, median pitch and duty, and the "loudest" track by
//! gain × duty, never by gain alone. One-shot **patches** are rusty's own rows:
//! zimmer leaves them out on purpose (an effect has no arrangement to be loudest
//! in), but a shooter's hundreds of one-shots are exactly the set this is for.
//!
//! **It counts, and stops.** No score, no grade, no diversity number, nothing that
//! can fail: a set of variations on one instrument is a legitimate thing to write on
//! purpose. A document that cannot be read or parsed is listed under `skipped`
//! rather than refusing the survey; a song track whose patch cannot be resolved
//! leaves its instrument columns absent (zimmer's rule).

mod lua;

use std::collections::BTreeMap;

use serde_json::Value as Json;
use zimmer::survey::{Register, SongSurvey, Survey};
use zimmer::{Patch, Song};

pub use lua::survey;

/// One one-shot patch, as its document states it — the patch half of a
/// [`zimmer::survey::TrackSurvey`]: what the instrument **is**.
#[derive(Debug, Clone, PartialEq)]
pub struct PatchSurvey {
    /// The path it was read from, or `#n` for the n-th inline document.
    pub name: String,
    /// The source kind, as the recipe spells it (`noise`, `karplus`, `fm2`, …).
    pub source: &'static str,
    /// The filter's kind (`lowpass`, …), absent when the patch has none.
    pub filter: Option<String>,
    /// The filter's base cutoff in Hz, absent when the patch has no filter.
    pub cutoff: Option<f32>,
    /// The **amp envelope's** sustain — not the source's: a `karplus` string or an
    /// `fm2` modulator decays on its own whatever this reads.
    pub sustain: f32,
}

impl PatchSurvey {
    fn of(name: String, patch: &Patch) -> Self {
        let filter = patch.filter.as_ref();
        Self {
            name,
            source: patch.source.kind(),
            filter: filter.and_then(|f| {
                serde_json::to_value(f.kind)
                    .ok()?
                    .as_str()
                    .map(str::to_owned)
            }),
            cutoff: filter.map(|f| f.cutoff),
            sustain: patch.amp.s,
        }
    }
}

/// One entry of the set: where it came from, and its JSON document (or why not).
pub type Document = (String, Result<Json, String>);

/// The whole set, sorted into patches and songs, plus what could not be read.
#[derive(Debug, Default)]
pub struct SetSurvey {
    pub patches: Vec<PatchSurvey>,
    pub songs: Survey,
    /// `(name, why)` for every document that could not be read or parsed.
    pub skipped: Vec<(String, String)>,
}

impl SetSurvey {
    /// Survey `documents` in order. A document with `tracks` is a song, anything
    /// else a patch; a song's named patches resolve with `resolve`.
    pub fn of(
        documents: impl IntoIterator<Item = Document>,
        resolve: &dyn zimmer::PatchResolver,
    ) -> Self {
        let mut set = Self::default();
        for (name, json) in documents {
            match json.and_then(|json| set.add(&name, json, resolve)) {
                Ok(()) => {}
                Err(why) => set.skipped.push((name, why)),
            }
        }
        set
    }

    fn add(
        &mut self,
        name: &str,
        json: Json,
        resolve: &dyn zimmer::PatchResolver,
    ) -> Result<(), String> {
        if json.get("tracks").is_some() {
            let song: Song =
                serde_json::from_value(json).map_err(|e| format!("invalid song: {e}"))?;
            self.songs.songs.push(SongSurvey::of(name, &song, resolve));
        } else {
            let patch: Patch =
                serde_json::from_value(json).map_err(|e| format!("invalid patch: {e}"))?;
            self.patches.push(PatchSurvey::of(name.to_owned(), &patch));
        }
        Ok(())
    }

    /// The set counted across: absent for fewer than two documents, where a
    /// summary would only repeat its one row.
    pub fn rollup(&self) -> Option<SetRollup> {
        if self.patches.len() + self.songs.songs.len() < 2 {
            return None;
        }
        let songs = self.songs.rollup();
        let mut rows: BTreeMap<&'static str, SourceRow> = BTreeMap::new();
        for count in &songs.sources {
            let row = rows
                .entry(count.source)
                .or_insert_with(|| SourceRow::new(count.source));
            (row.songs, row.loudest) = (count.songs, count.loudest);
        }
        for patch in &self.patches {
            let row = rows
                .entry(patch.source)
                .or_insert_with(|| SourceRow::new(patch.source));
            row.patches += 1;
            if let Some(hz) = patch.cutoff {
                row.cutoff = Some(
                    row.cutoff
                        .map_or((hz, hz), |(lo, hi)| (lo.min(hz), hi.max(hz))),
                );
            }
        }
        let mut sources: Vec<SourceRow> = rows.into_values().collect();
        // Most-used first, so the row a reader is looking for is the top one; the
        // name breaks a tie, so the order is the same on every run.
        sources.sort_by(|a, b| {
            (b.patches + b.songs)
                .cmp(&(a.patches + a.songs))
                .then(b.loudest.cmp(&a.loudest))
                .then(a.source.cmp(b.source))
        });
        Some(SetRollup {
            patches: self.patches.len(),
            songs: songs.songs,
            sources,
            tempo: songs.tempo,
            register: songs.register,
        })
    }
}

/// How often one source kind turns up across the set.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceRow {
    pub source: &'static str,
    /// How many one-shot patches use it.
    pub patches: usize,
    /// How many songs have at least one track using it.
    pub songs: usize,
    /// How many songs have it in their loudest track (gain × duty).
    pub loudest: usize,
    /// The lowest and highest base cutoff among the patches using it.
    pub cutoff: Option<(f32, f32)>,
}

impl SourceRow {
    fn new(source: &'static str) -> Self {
        Self {
            source,
            patches: 0,
            songs: 0,
            loudest: 0,
            cutoff: None,
        }
    }
}

/// The set, counted: per-source rows, and the spread of tempos and registers.
#[derive(Debug, Clone, PartialEq)]
pub struct SetRollup {
    pub patches: usize,
    pub songs: usize,
    pub sources: Vec<SourceRow>,
    /// Slowest and fastest song tempo; absent with no songs.
    pub tempo: Option<(f32, f32)>,
    /// Lowest and highest note any song plays; absent when none plays one.
    pub register: Option<Register>,
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
