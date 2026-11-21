//! A [`Score`] into a [`Song`]: the half of the import that has to decide how
//! each fact in the file is written in the document.
//!
//! Every decision here is about *notation*, never about the music. A part is
//! a track, a tempo event is a jump in the map, a note is a note — each moved
//! across as the file had it. What is chosen is how it reads: bar-numbered
//! patterns, note names spelled by the key signature, a velocity to three
//! places.

use std::collections::{BTreeMap, BTreeSet};

use super::bars::{self, Phrase};
use super::read::{Held, Part, Score, Unread};
use super::{Imported, MidiError, sound};
use crate::song::{Note, PatchRef, Pattern, Pitch, Song, TempoChange, Track};

/// The tempo a file that never writes one is in, by the MIDI specification.
const DEFAULT_BPM: f32 = 120.0;

/// Writes `score` as a song, with the sentences saying what it left behind.
pub(super) fn song(score: Score) -> Result<Imported, MidiError> {
    if score.parts.is_empty() {
        return Err(MidiError::NoNotes);
    }
    let ppq = f64::from(score.ppq);
    let end = score
        .parts
        .iter()
        .flat_map(|part| &part.notes)
        .map(|note| note.end)
        .max()
        .unwrap_or(1);
    let phrases = bars::phrases(&score.meters, score.ppq, end).ok_or(MidiError::TooLong)?;

    let mut keys = score.keys.clone();
    keys.sort_by_key(|key| key.0);
    // A format-1 file often restates the signature at the top of every track;
    // only a signature that differs from the one before it is a change.
    keys.dedup_by_key(|key| (key.1, key.2));
    let first_key = keys.first().copied();
    let flats = first_key.is_some_and(|(_, sharps, _)| sharps < 0);

    let names = track_names(&score.parts);
    let gain = equal_gain(score.parts.len());
    let tracks = score
        .parts
        .iter()
        .zip(&names)
        .map(|(part, name)| Track {
            name: name.clone(),
            patch: PatchRef::Inline(Box::new(if part.is_drums() {
                sound::drums()
            } else {
                sound::pitched()
            })),
            gain,
            pan: 0.0,
            fx: vec![],
            send: 1.0,
            push: 0.0,
            swing: None,
            humanize: None,
        })
        .collect();

    let mut patterns: Vec<Pattern> = phrases
        .iter()
        .map(|phrase| Pattern {
            beats: ((phrase.end - phrase.start) / ppq) as f32,
            notes: Vec::new(),
        })
        .collect();
    for (part, name) in score.parts.iter().zip(&names) {
        for held in &part.notes {
            let at = phrases.partition_point(|phrase| phrase.start <= held.start as f64) - 1;
            let note = note(name, part, held, &phrases[at], ppq, flats);
            patterns[at].notes.push(note.into());
        }
    }

    let (bpm, tempo) = tempo_map(&score.tempos, ppq);
    let key = first_key.and_then(|(_, sharps, minor)| sound::key_name(sharps, minor));
    let song = Song {
        bpm,
        tempo,
        seed: 0,
        key,
        tracks,
        arrangement: phrases.iter().map(|phrase| phrase.name().into()).collect(),
        patterns: phrases
            .iter()
            .map(Phrase::name)
            .zip(patterns)
            .collect::<BTreeMap<_, _>>(),
        swing: 0.0,
        humanize: None,
        fx: vec![],
        automation: vec![],
        fit: None,
        anchors: Vec::new(),
        fade: None,
        tail: None,
    };
    let left_out = left_out(&score, &names, &keys);
    Ok(Imported { song, left_out })
}

/// One held note as a pattern entry, timed from its phrase's start.
fn note(track: &str, part: &Part, held: &Held, phrase: &Phrase, ppq: f64, flats: bool) -> Note {
    Note {
        track: track.to_owned(),
        // A drum key is an instrument number, not a pitch — 36 is a kick
        // because General MIDI says so — so it stays the number the file
        // wrote rather than becoming a note name nobody would read as a drum.
        note: if part.is_drums() {
            Pitch::Midi(f32::from(held.key))
        } else {
            Pitch::Name(sound::spell(held.key, flats))
        },
        start: ((held.start as f64 - phrase.start) / ppq) as f32,
        dur: ((held.end - held.start) as f64 / ppq) as f32,
        // Three places is finer than MIDI's own step of 1/127, so nothing the
        // file said is lost, and `0.787` reads where `0.78740156` does not.
        vel: (f32::from(held.vel) / 127.0 * 1000.0).round() / 1000.0,
        articulation: None,
    }
}

/// Every track at one level: `1/√n`, so the parts sum to about the level one
/// would have alone and the limiter is not flattening the first bake.
///
/// Equal because the file has no opinion about balance that the importer
/// reads — a volume controller is a performance gesture, not a mix — and an
/// unequal default would be a guess dressed as one.
fn equal_gain(tracks: usize) -> f32 {
    let gain = 1.0 / (tracks as f32).sqrt();
    (gain * 100.0).round() / 100.0
}

/// The opening tempo and the jumps after it.
///
/// Of two tempo events on one tick the later-written wins, and an event that
/// restates the tempo already in force is dropped: it changes nothing a
/// listener could hear and would be a line in the map for nothing.
fn tempo_map(tempos: &[(u64, u32)], ppq: f64) -> (f32, Vec<TempoChange>) {
    let mut sorted = tempos.to_vec();
    sorted.sort_by_key(|tempo| tempo.0);
    let mut by_tick: BTreeMap<u64, f32> = BTreeMap::new();
    for (tick, micros) in sorted {
        // A thousandth of a beat per minute: `60e6 / 461538` is 130.0001, and
        // the fourth place is the file's integer rounding rather than a tempo.
        let bpm = (60_000_000.0 / f64::from(micros) * 1000.0).round() / 1000.0;
        by_tick.insert(tick, bpm as f32);
    }
    let bpm = by_tick.get(&0).copied().unwrap_or(DEFAULT_BPM);
    let mut current = bpm;
    let mut changes = Vec::new();
    for (&tick, &next) in by_tick.range(1..) {
        if next != current {
            changes.push(TempoChange::jump((tick as f64 / ppq) as f32, next));
            current = next;
        }
    }
    (bpm, changes)
}

/// One name per part: the track's own, else `track-N` (or `drums`), with the
/// channel added when one MIDI track plays on several, and a number when two
/// parts would still share one.
fn track_names(parts: &[Part]) -> Vec<String> {
    let mut channels: BTreeMap<usize, usize> = BTreeMap::new();
    for part in parts {
        *channels.entry(part.track).or_default() += 1;
    }
    let mut taken = BTreeSet::new();
    parts
        .iter()
        .map(|part| {
            let mut base = part.name.clone().unwrap_or_else(|| {
                if part.is_drums() {
                    "drums".to_owned()
                } else {
                    format!("track-{}", part.track + 1)
                }
            });
            if channels[&part.track] > 1 {
                base = format!("{base}-ch{}", part.channel + 1);
            }
            let (mut name, mut n) = (base.clone(), 1);
            while !taken.insert(name.clone()) {
                n += 1;
                name = format!("{base}-{n}");
            }
            name
        })
        .collect()
}

/// What the file said that the song does not, one sentence each.
fn left_out(score: &Score, names: &[String], keys: &[(u64, i8, bool)]) -> Vec<String> {
    let Unread {
        controllers,
        bends,
        aftertouch,
        sysex,
        unreleased,
        instant,
    } = score.unread;
    let programs: Vec<String> = score
        .parts
        .iter()
        .zip(names)
        .filter_map(|(part, name)| {
            let program = part.program.filter(|_| !part.is_drums())?;
            Some(format!("`{name}` program {}", u16::from(program) + 1))
        })
        .collect();
    let mut said = Vec::new();
    let mut count = |n: usize, what: &str| {
        if n > 0 {
            said.push(format!("{n} {what}"));
        }
    };
    count(
        controllers,
        "controller changes (sustain pedal, volume, pan and the like) are not imported",
    );
    count(
        bends,
        "pitch bends are not imported — every note plays at its written pitch",
    );
    count(aftertouch, "aftertouch messages are not imported");
    count(sysex, "system-exclusive messages are not imported");
    count(
        unreleased,
        "notes were never released in the file; each ends where its track does",
    );
    count(
        instant,
        "notes were released on the tick they started; each is given one tick",
    );
    if !programs.is_empty() {
        said.push(format!(
            "the file asks for General MIDI instruments ({}), and a program number is not \
             a sound here",
            programs.join(", ")
        ));
    }
    if let Some(&(_, sharps, minor)) = keys.first()
        && sound::key_name(sharps, minor).is_none()
    {
        said.push(format!(
            "a key signature of {sharps} accidentals is not a key, so the song declares none"
        ));
    }
    if keys.len() > 1 {
        said.push(format!(
            "the key signature changes {} more time(s); `key` is the first, and every note \
             is written as a pitch, so nothing sounds different",
            keys.len() - 1
        ));
    }
    said
}
