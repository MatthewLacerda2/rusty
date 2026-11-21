//! One pass over the arrangement: each note placed, played and rendered into the mix.

use rusty_core::collections::Map;

use super::mix::Mix;
use super::PatchResolver;
use crate::core::lfo::Onset;
use crate::core::{self, RATE};
use crate::error::{NoteAt, SynthError};
use crate::hash::hash3;
use crate::note::{Glide, NoteOpts};
use crate::patch::Patch;
use crate::song::automate::{self, Automation, Riding};
use crate::song::clock::Clock;
use crate::song::excerpt::{Excerpt, Scope};
use crate::song::play::articulation::Stroke;
use crate::song::play::feel::{swung, Played};
use crate::song::play::glide::{Slides, Trail};
use crate::song::shape::plan;
use crate::song::{Articulation, Key, Layer, Note, PatchRef, Song};

pub(super) struct Walk<'s> {
    song: &'s Song,
    patches: Vec<Patch>,
    riding: Vec<Riding<'s>>,
    pub(super) clock: Clock,
    passes: u32,
    pub(super) scope: Scope,
    key: Option<Key>,
    voiced: Map<&'s str, Vec<Note>>,
    track_index: Map<&'s str, usize>,
    slides: Map<&'s str, Slides>,
    played: Vec<Played>,
}

struct At<'a> {
    layer: &'a Layer,
    sliding: Option<&'a Slides>,
    index: usize,
    note: &'a Note,
    cursor_beats: f32,
    place: u64,
}

struct Struck {
    pitch: f32,
    opts: NoteOpts,
    beat_at: f32,
    at: usize,
}

impl<'s> Walk<'s> {
    pub(super) fn new(
        song: &'s Song,
        resolve: &dyn PatchResolver,
        excerpt: &Excerpt,
    ) -> Result<Self, SynthError> {
        song.validate()?;
        let patches = resolve_patches(song, resolve)?;
        crate::song::validate::check_resolved(song, &patches)?;
        let riding = automate::riding(song);
        let (clock, passes) = plan(song);
        let scope = Scope::of(song, excerpt, &clock)?;
        let key = song.key()?;
        let voiced: Map<&str, Vec<Note>> = song
            .patterns
            .iter()
            .map(|(name, pattern)| Ok((name.as_str(), pattern.voices(key.as_ref())?)))
            .collect::<Result<_, SynthError>>()?;
        let track_index: Map<&str, usize> = song
            .tracks
            .iter()
            .enumerate()
            .map(|(index, track)| (track.name.as_str(), index))
            .collect();
        let slides: Map<&str, Slides> = if glides(&voiced) {
            voiced
                .iter()
                .map(|(name, notes)| {
                    Ok((*name, Slides::of(notes, &track_index, song.tracks.len())?))
                })
                .collect::<Result<_, SynthError>>()?
        } else {
            Map::default()
        };
        let played = song.tracks.iter().map(|it| Played::of(song, it)).collect();
        Ok(Self {
            song,
            patches,
            riding,
            clock,
            passes,
            scope,
            key,
            voiced,
            track_index,
            slides,
            played,
        })
    }

    pub(super) fn arrangement_end(&self) -> usize {
        let played = self.song.arrangement_beats() * self.passes as f32;
        (self.clock.seconds(played) * RATE).round() as usize
    }

    /// `false` when `stop` cut the walk short.
    pub(super) fn run(&self, mix: &mut Mix, stop: &dyn Fn() -> bool) -> Result<bool, SynthError> {
        let song = self.song;
        let mut trail = Trail::new(song.tracks.len());
        let mut cursor_beats = 0.0f32;
        let mut ordinal: u64 = 0;
        for entry in song
            .arrangement
            .iter()
            .cycle()
            .take(song.arrangement.len() * self.passes as usize)
        {
            for layer in entry.layers() {
                if !song.patterns.contains_key(layer.pattern()) {
                    return Err(SynthError::UnknownPattern {
                        pattern: layer.pattern().to_owned(),
                    });
                }
                let sliding = self.slides.get(layer.pattern());
                for (index, note) in self
                    .voiced
                    .get(layer.pattern())
                    .into_iter()
                    .flatten()
                    .enumerate()
                {
                    // A silenced track still consumes its ordinal, so muting one for eight bars does
                    // not re-roll the noise of every note after it.
                    let place = ordinal;
                    ordinal += 1;
                    let at = At {
                        layer,
                        sliding,
                        index,
                        note,
                        cursor_beats,
                        place,
                    };
                    if !self.note(mix, &trail, &at, stop)? {
                        return Ok(false);
                    }
                }
            }
            for layer in entry.layers() {
                if let Some(slides) = self.slides.get(layer.pattern()) {
                    trail.advance(slides, layer, self.key.as_ref());
                }
            }
            cursor_beats += song.slot_beats(entry).unwrap_or_default();
        }
        Ok(true)
    }

    fn note(
        &self,
        mix: &mut Mix,
        trail: &Trail,
        at: &At,
        stop: &dyn Fn() -> bool,
    ) -> Result<bool, SynthError> {
        let note = at.note;
        let track = self.track_index[note.track.as_str()];
        if !at.layer.plays(&note.track) {
            return Ok(true);
        }
        if !mix.needs(track) {
            return Ok(true);
        }
        let struck = self.strike(trail, at, track)?;
        if !self.scope.reaches(struck.at) {
            return Ok(true);
        }
        if stop() {
            return Ok(false);
        }
        let instrument = &self.patches[track];
        let swept = self.riding[track]
            .cutoff
            .zip(instrument.filter)
            .map(|(curve, filter)| sweep(curve, &self.clock, struck.beat_at, filter.cutoff));
        let onset = Onset {
            grid: &self.clock,
            frame: struck.at,
        };
        let rendered = core::render_swept(
            instrument,
            struck.pitch,
            &struck.opts,
            swept.as_ref().map(|sweep| sweep as &dyn Fn(usize) -> f32),
            Some(&onset),
        )
        .map_err(|error| {
            error.at(NoteAt {
                pattern: at.layer.pattern().to_owned(),
                track: note.track.clone(),
                start: note.start,
            })
        })?;
        mix.add(track, &rendered, struck.at);
        Ok(true)
    }

    fn strike(&self, trail: &Trail, at: &At, track: usize) -> Result<Struck, SynthError> {
        let (song, note, layer, place) = (self.song, at.note, at.layer, at.place);
        let Played {
            swing,
            humanize: feel,
            push,
        } = self.played[track];
        let seed = note_seed(song.seed, track, place);
        let key = self.key.as_ref();
        let stroke = Stroke::of(note.articulation);
        let written = note.vel * layer.vel_scale() * stroke.velocity;
        let velocity = feel.velocity(written, track, place, song.seed);
        let beat_at = at.cursor_beats + swung(note.start, swing);
        let gate = self.clock.span(beat_at, note.dur * stroke.gate);
        let pitch = layer.played_pitch(note.note.to_midi()?, key);
        let glide =
            match (stroke.slide_seconds(gate), at.sliding) {
                (Some(seconds), Some(slides)) => trail
                    .from(slides, at.index, track, layer, key)
                    .map(|from| Glide {
                        semitones: from - pitch,
                        seconds,
                    }),
                _ => None,
            };
        let opts = NoteOpts {
            duration: gate,
            velocity,
            timbre: stroke.timbre(velocity) + feel.timbre(velocity, track, place, song.seed),
            glide,
            seed,
        };
        let onset = self.clock.seconds(beat_at)
            + stroke.onset_seconds
            + feel.onset_seconds(track, place, song.seed)
            + push;
        let at = (onset * RATE).round().max(0.0) as usize;
        Ok(Struck {
            pitch,
            opts,
            beat_at,
            at,
        })
    }
}

fn sweep<'a>(
    curve: &'a Automation,
    clock: &'a Clock,
    beat: f32,
    fallback: f32,
) -> impl Fn(usize) -> f32 + 'a {
    let onset = (clock.seconds(beat) * RATE).round().max(0.0) as usize;
    move |frame| {
        curve
            .value_at(clock.beat_at(onset + frame))
            .unwrap_or(fallback)
    }
}

fn glides(voiced: &Map<&str, Vec<Note>>) -> bool {
    voiced
        .values()
        .flatten()
        .any(|note| note.articulation == Some(Articulation::Glide))
}

fn note_seed(song_seed: u64, track: usize, ordinal: u64) -> u64 {
    let hi = u64::from(hash3(track as i64, ordinal as i64, 0, song_seed));
    let lo = u64::from(hash3(track as i64, ordinal as i64, 1, song_seed));
    (hi << 32) | lo
}

fn resolve_patches(song: &Song, resolve: &dyn PatchResolver) -> Result<Vec<Patch>, SynthError> {
    song.tracks
        .iter()
        .map(|track| match &track.patch {
            PatchRef::Inline(patch) => Ok((**patch).clone()),
            PatchRef::Named(reference) => {
                resolve
                    .resolve(reference)
                    .map_err(|reason| SynthError::UnresolvedPatch {
                        track: track.name.clone(),
                        reference: reference.clone(),
                        reason,
                    })
            }
        })
        .collect()
}
