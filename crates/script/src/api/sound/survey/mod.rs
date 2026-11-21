mod lua;

use std::collections::BTreeMap;

use rusty_zimmer::survey::{Register, SongSurvey, Survey};
use rusty_zimmer::{Patch, Song};
use serde::de::DeserializeOwned;
use serde_json::Value as Json;

use crate::api::lua_json::decode;

pub use lua::survey;

#[derive(Debug, Clone, PartialEq)]
pub struct PatchSurvey {
    pub name: String,
    pub source: &'static str,
    pub filter: Option<String>,
    pub cutoff: Option<f32>,
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

pub type Document = (String, Result<Json, String>, Origin);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    File,
    Inline,
}

impl Origin {
    fn decode<T: DeserializeOwned>(self, json: Json) -> Result<T, String> {
        match self {
            Self::File => serde_json::from_value(json).map_err(|e| e.to_string()),
            Self::Inline => decode(json),
        }
    }
}

#[derive(Debug, Default)]
pub struct SetSurvey {
    pub patches: Vec<PatchSurvey>,
    pub songs: Survey,
    pub skipped: Vec<(String, String)>,
}

impl SetSurvey {
    pub fn of(
        documents: impl IntoIterator<Item = Document>,
        resolve: &dyn rusty_zimmer::PatchResolver,
    ) -> Self {
        let mut set = Self::default();
        for (name, json, origin) in documents {
            match json.and_then(|json| set.add(&name, json, origin, resolve)) {
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
        origin: Origin,
        resolve: &dyn rusty_zimmer::PatchResolver,
    ) -> Result<(), String> {
        if json.get("tracks").is_some() {
            let song: Song = origin
                .decode(json)
                .map_err(|e| format!("invalid song: {e}"))?;
            self.songs.songs.push(SongSurvey::of(name, &song, resolve));
        } else {
            let patch: Patch = origin
                .decode(json)
                .map_err(|e| format!("invalid patch: {e}"))?;
            self.patches.push(PatchSurvey::of(name.to_owned(), &patch));
        }
        Ok(())
    }

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

#[derive(Debug, Clone, PartialEq)]
pub struct SourceRow {
    pub source: &'static str,
    pub patches: usize,
    pub songs: usize,
    pub loudest: usize,
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

#[derive(Debug, Clone, PartialEq)]
pub struct SetRollup {
    pub patches: usize,
    pub songs: usize,
    pub sources: Vec<SourceRow>,
    pub tempo: Option<(f32, f32)>,
    pub register: Option<Register>,
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

#[cfg(test)]
#[path = "origin_tests.rs"]
mod origin_tests;
