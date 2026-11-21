//! Playing a song through into the notes a MIDI file holds: per track, in
//! ticks, at whole keys and velocities.
//!
//! The walk is the renderer's — the arrangement in order, each pattern's
//! entries expanded to notes, each entry's transforms applied to the written
//! pattern — and stops where the renderer starts making sound. So a note
//! lands on the tick the bake puts it on, before anything about seconds.

use std::collections::HashMap;

use super::TICKS_PER_BEAT;
use super::drums::Voice;
use crate::SynthError;
use crate::song::articulation::Stroke;
use crate::song::feel::swung;
use crate::song::{Articulation, Note, Song};

/// The percussion channel as the wire counts it — channel 10 to a musician.
pub(super) const DRUM_CHANNEL: u8 = 9;

/// The fifteen channels that are not the drums', in the order tracks take
/// them.
const PITCHED: [u8; 15] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 10, 11, 12, 13, 14, 15];

/// A song, played: one [`Part`] per song track, and what did not fit.
#[derive(Debug)]
pub(super) struct Played<'s> {
    pub(super) parts: Vec<Part<'s>>,
    pub(super) unsaid: Unsaid,
}

/// One song track as one MIDI track.
#[derive(Debug)]
pub(super) struct Part<'s> {
    /// The song track's name, which the MIDI track is given.
    pub(super) name: &'s str,
    /// Which channel it plays on, as the wire counts.
    pub(super) channel: u8,
    /// Its notes, in the order they were played.
    pub(super) notes: Vec<Struck>,
}

/// One note as MIDI has it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Struck {
    pub(super) start: u64,
    pub(super) end: u64,
    pub(super) key: u8,
    pub(super) vel: u8,
}

/// Counts of what the song played and the file cannot say exactly.
#[derive(Debug, Default)]
pub(super) struct Unsaid {
    glides: usize,
    microtonal: usize,
    off_keyboard: usize,
    silent: usize,
    too_loud: usize,
    shared_channels: usize,
    humanize: bool,
    pushed: bool,
    fit: bool,
}

/// Plays `song` through, each track written as `voices` says.
pub(super) fn parts<'s>(song: &'s Song, voices: &[Voice]) -> Result<Played<'s>, SynthError> {
    let key = song.key()?;
    let voiced: HashMap<&str, Vec<Note>> = song
        .patterns
        .iter()
        .map(|(name, pattern)| Ok((name.as_str(), pattern.voices(key.as_ref())?)))
        .collect::<Result<_, SynthError>>()?;
    let index: HashMap<&str, usize> = song
        .tracks
        .iter()
        .enumerate()
        .map(|(at, track)| (track.name.as_str(), at))
        .collect();

    let mut unsaid = Unsaid {
        humanize: song.humanize.is_some_and(|feel| feel != Default::default())
            || (song.tracks.iter())
                .any(|it| it.humanize.is_some_and(|feel| feel != Default::default())),
        pushed: song.tracks.iter().any(|it| it.push != 0.0),
        fit: song.fit.is_some(),
        ..Unsaid::default()
    };
    let mut pitched = 0;
    let mut parts: Vec<Part<'s>> = song
        .tracks
        .iter()
        .zip(voices)
        .map(|(track, voice)| Part {
            name: &track.name,
            channel: match voice {
                Voice::Drum(_) => DRUM_CHANNEL,
                Voice::Pitched => {
                    pitched += 1;
                    PITCHED[(pitched - 1) % PITCHED.len()]
                }
            },
            notes: Vec::new(),
        })
        .collect();
    unsaid.shared_channels = pitched.saturating_sub(PITCHED.len());

    let mut cursor = 0.0_f64;
    for entry in &song.arrangement {
        let Some(slot) = song.slot_beats(entry) else {
            continue;
        };
        for (layer, note) in entry.layers().iter().flat_map(|layer| {
            voiced
                .get(layer.pattern())
                .into_iter()
                .flatten()
                .map(move |note| (layer, note))
        }) {
            if !layer.plays(&note.track) {
                continue;
            }
            let track = index[note.track.as_str()];
            let stroke = Stroke::of(note.articulation);
            if note.articulation == Some(Articulation::Glide) {
                unsaid.glides += 1;
            }
            let pitch = layer.played_pitch(note.note.to_midi()?, key.as_ref());
            let Some(key) = keyed(pitch, voices[track], &mut unsaid) else {
                continue;
            };
            let Some(vel) = velocity(note.vel * layer.vel_scale() * stroke.velocity, &mut unsaid)
            else {
                continue;
            };
            let swing = song.tracks[track].swing.unwrap_or(song.swing);
            let onset = cursor + f64::from(swung(note.start, swing));
            let held = f64::from(note.dur * stroke.gate);
            let start = ticks(onset);
            parts[track].notes.push(Struck {
                start,
                end: ticks(onset + held).max(start + 1),
                key,
                vel,
            });
        }
        cursor += f64::from(slot);
    }
    Ok(Played { parts, unsaid })
}

/// A beat as a tick, to the nearest.
fn ticks(beats: f64) -> u64 {
    (beats * f64::from(TICKS_PER_BEAT)).round().max(0.0) as u64
}

/// The key a played pitch is written on, or `None` for one off the keyboard.
fn keyed(pitch: f32, voice: Voice, unsaid: &mut Unsaid) -> Option<u8> {
    if let Voice::Drum(Some(key)) = voice {
        return Some(key);
    }
    let key = pitch.round();
    if !(0.0..=127.0).contains(&key) {
        unsaid.off_keyboard += 1;
        return None;
    }
    if (pitch - key).abs() > 0.001 {
        unsaid.microtonal += 1;
    }
    Some(key as u8)
}

/// A played velocity as MIDI's `1..=127`, or `None` for a note that makes no
/// sound — which MIDI cannot write, since a note-on at zero is a release.
fn velocity(played: f32, unsaid: &mut Unsaid) -> Option<u8> {
    if played <= 0.0 {
        unsaid.silent += 1;
        return None;
    }
    let vel = (played * 127.0).round();
    if vel > 127.0 {
        unsaid.too_loud += 1;
    }
    Some(vel.clamp(1.0, 127.0) as u8)
}

impl Unsaid {
    /// One sentence per kind of thing lost, in the order a reader would care.
    pub(super) fn sentences(&self) -> Vec<String> {
        let mut said = Vec::new();
        let mut count = |n: usize, what: &str| {
            if n > 0 {
                said.push(format!("{n} {what}"));
            }
        };
        count(
            self.off_keyboard,
            "notes are pitched outside MIDI's 0–127 and are not written",
        );
        count(
            self.microtonal,
            "notes are between two keys and are written on the nearer one",
        );
        count(
            self.glides,
            "glides are written as plain notes — a slide would be a pitch bend",
        );
        count(
            self.too_loud,
            "notes are struck harder than MIDI's top velocity and are written at 127",
        );
        count(
            self.silent,
            "notes play at velocity zero and are not written — MIDI has no silent note",
        );
        count(
            self.shared_channels,
            "tracks share a channel with an earlier one: MIDI has fifteen besides the drums'",
        );
        if self.humanize {
            said.push(
                "`humanize` is not written: the file is the part as composed, and a DAW's own \
                 humanize can be applied to it"
                    .to_owned(),
            );
        }
        if self.pushed {
            said.push(
                "a track's `push` is not written: the file is on the grid, and a DAW's own track \
                 delay can sit a part ahead of the beat or behind it"
                    .to_owned(),
            );
        }
        if self.fit {
            said.push(
                "`fit` is not applied: the file plays the arrangement once, at the written tempo"
                    .to_owned(),
            );
        }
        said
    }
}
