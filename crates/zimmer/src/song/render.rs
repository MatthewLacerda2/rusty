//! Walk the arrangement, sum the notes, limit the result.

use std::collections::HashMap;

use super::articulation::Stroke;
use super::automate::{self, Automation};
use super::clock::Clock;
use super::excerpt::{Excerpt, Scope};
use super::feel::{Played, swung};
use super::glide::{Slides, Trail};
use super::mix::Mix;
use super::shape::{limit, plan, shape};
use super::{Articulation, Note, PatchRef, Song, sections};
use crate::core::lfo::Onset;
use crate::core::{self, RATE};
use crate::error::{NoteAt, SynthError};
use crate::hash::hash3;
use crate::level::{Cut, Layer};
use crate::note::{Glide, NoteOpts};
use crate::patch::Patch;
use crate::stereo::Stereo;

pub trait PatchResolver {
    fn resolve(&self, reference: &str) -> Result<Patch, String>;
}

impl<F> PatchResolver for F
where
    F: Fn(&str) -> Result<Patch, String>,
{
    fn resolve(&self, reference: &str) -> Result<Patch, String> {
        self(reference)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct InlineOnly;

impl PatchResolver for InlineOnly {
    fn resolve(&self, _reference: &str) -> Result<Patch, String> {
        Err("this song must carry its patches inline".to_owned())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Mixdown {
    pub(crate) master: Stereo,
    pub(crate) tracks: Vec<Layer>,
    pub(crate) sections: Vec<Cut>,
}

pub fn render_song(song: &Song, resolve: &dyn PatchResolver) -> Result<Vec<f32>, SynthError> {
    render_excerpt(song, resolve, &Excerpt::default())
}

pub fn render_excerpt(
    song: &Song,
    resolve: &dyn PatchResolver,
    excerpt: &Excerpt,
) -> Result<Vec<f32>, SynthError> {
    let mixed = mix_song(song, resolve, excerpt, &never)?;
    Ok(mixed
        .expect("a render nobody can stop runs to the end")
        .master
        .interleaved())
}

pub(crate) fn never() -> bool {
    false
}

pub(crate) fn mix_song(
    song: &Song,
    resolve: &dyn PatchResolver,
    excerpt: &Excerpt,
    stop: &dyn Fn() -> bool,
) -> Result<Option<Mixdown>, SynthError> {
    song.validate()?;
    let patches = resolve_patches(song, resolve)?;
    super::validate::check_resolved(song, &patches)?;
    let riding = automate::riding(song);
    let (clock, passes) = plan(song);
    let scope = Scope::of(song, excerpt, &clock)?;
    let key = song.key()?;
    let voiced: HashMap<&str, Vec<Note>> = song
        .patterns
        .iter()
        .map(|(name, pattern)| Ok((name.as_str(), pattern.voices(key.as_ref())?)))
        .collect::<Result<_, SynthError>>()?;
    let track_index: HashMap<&str, usize> = song
        .tracks
        .iter()
        .enumerate()
        .map(|(index, track)| (track.name.as_str(), index))
        .collect();
    let slides: HashMap<&str, Slides> = if glides(&voiced) {
        voiced
            .iter()
            .map(|(name, notes)| Ok((*name, Slides::of(notes, &track_index, song.tracks.len())?)))
            .collect::<Result<_, SynthError>>()?
    } else {
        HashMap::new()
    };
    let mut trail = Trail::new(song.tracks.len());

    let played = song.arrangement_beats() * passes as f32;
    let arrangement_end = (clock.seconds(played) * RATE).round() as usize;
    let mut mix = Mix::new(song, arrangement_end, &clock, &scope);
    let mut cursor_beats = 0.0f32;
    let mut ordinal: u64 = 0;
    let played: Vec<Played> = song.tracks.iter().map(|it| Played::of(song, it)).collect();

    for entry in song
        .arrangement
        .iter()
        .cycle()
        .take(song.arrangement.len() * passes as usize)
    {
        for layer in entry.layers() {
            if !song.patterns.contains_key(layer.pattern()) {
                return Err(SynthError::UnknownPattern {
                    pattern: layer.pattern().to_owned(),
                });
            }
            let sliding = slides.get(layer.pattern());
            for (index, note) in voiced
                .get(layer.pattern())
                .into_iter()
                .flatten()
                .enumerate()
            {
                // A silenced track still consumes its ordinal, so muting one for eight bars does
                // not re-roll the noise of every note after it.
                let track = track_index[note.track.as_str()];
                let Played {
                    swing,
                    humanize: feel,
                    push,
                } = played[track];
                let place = ordinal;
                let seed = note_seed(song.seed, track, place);
                ordinal += 1;
                if !layer.plays(&note.track) {
                    continue;
                }
                if !mix.needs(track) {
                    continue;
                }
                let stroke = Stroke::of(note.articulation);
                let written = note.vel * layer.vel_scale() * stroke.velocity;
                let velocity = feel.velocity(written, track, place, song.seed);
                let beat_at = cursor_beats + swung(note.start, swing);
                let gate = clock.span(beat_at, note.dur * stroke.gate);
                let pitch = layer.played_pitch(note.note.to_midi()?, key.as_ref());
                let glide = match (stroke.slide_seconds(gate), sliding) {
                    (Some(seconds), Some(slides)) => trail
                        .from(slides, index, track, layer, key.as_ref())
                        .map(|from| Glide {
                            semitones: from - pitch,
                            seconds,
                        }),
                    _ => None,
                };
                let opts = NoteOpts {
                    duration: gate,
                    velocity,
                    timbre: stroke.timbre(velocity)
                        + feel.timbre(velocity, track, place, song.seed),
                    glide,
                    seed,
                };
                let onset = clock.seconds(beat_at)
                    + stroke.onset_seconds
                    + feel.onset_seconds(track, place, song.seed)
                    + push;
                let at = (onset * RATE).round().max(0.0) as usize;
                if !scope.reaches(at) {
                    continue;
                }
                if stop() {
                    return Ok(None);
                }
                let instrument = &patches[track];
                let swept = riding[track]
                    .cutoff
                    .zip(instrument.filter)
                    .map(|(curve, filter)| sweep(curve, &clock, beat_at, filter.cutoff));
                let onset = Onset {
                    grid: &clock,
                    frame: at,
                };
                let rendered = core::render_swept(
                    instrument,
                    pitch,
                    &opts,
                    swept.as_ref().map(|sweep| sweep as &dyn Fn(usize) -> f32),
                    Some(&onset),
                )
                .map_err(|error| {
                    error.at(NoteAt {
                        pattern: layer.pattern().to_owned(),
                        track: note.track.clone(),
                        start: note.start,
                    })
                })?;
                mix.add(track, &rendered, at);
            }
        }
        for layer in entry.layers() {
            if let Some(slides) = slides.get(layer.pattern()) {
                trail.advance(slides, layer, key.as_ref());
            }
        }
        cursor_beats += song.slot_beats(entry).unwrap_or_default();
    }

    let sections = sections::of(song, scope.opens_at_seconds());
    if stop() {
        return Ok(None);
    }
    let (mut master, tracks) = mix.finish(&sections);
    limit(song, &mut master, arrangement_end)?;
    // Then length and level, in that order, on the limited signal.
    shape(song, &mut master, arrangement_end);
    let (from, to) = scope.keep(master.frames());
    master.cut(from, to);
    Ok(Some(Mixdown {
        master,
        tracks,
        sections,
    }))
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

fn glides(voiced: &HashMap<&str, Vec<Note>>) -> bool {
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
