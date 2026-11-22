//! Scientific pitch notation in equal temperament: A4 = 440 Hz = MIDI 69, and C4 (middle C) is 60.

use crate::error::SynthError;

/// MIDI note numbers, `C-1` to `G9`.
pub(crate) const MIDI_RANGE: (f32, f32) = (0.0, 127.0);

const A4_MIDI: f32 = 69.0;
const A4_HZ: f32 = 440.0;

const NATURALS: [(char, i32); 7] = [
    ('c', 0),
    ('d', 2),
    ('e', 4),
    ('f', 5),
    ('g', 7),
    ('a', 9),
    ('b', 11),
];

/// Semitones above the major mode's root; `song::key` rotates it into the other modes.
pub(crate) const DIATONIC: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glide {
    pub semitones: f32,
    pub seconds: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NoteOpts {
    pub duration: f32,
    pub velocity: f32,
    pub timbre: f32,
    pub glide: Option<Glide>,
    pub seed: u64,
}

impl Default for NoteOpts {
    fn default() -> Self {
        Self {
            duration: 0.5,
            velocity: 0.8,
            timbre: 0.0,
            glide: None,
            seed: 0,
        }
    }
}

pub fn parse_note(name: &str) -> Result<f32, SynthError> {
    let text = name.trim().to_ascii_lowercase();
    let mut chars = text.chars();
    let letter = chars.next().ok_or(SynthError::EmptyNoteName)?;
    let semitone = NATURALS
        .iter()
        .find(|(natural, _)| *natural == letter)
        .map(|(_, semitone)| *semitone)
        .ok_or_else(|| SynthError::BadNoteLetter {
            name: name.to_owned(),
            letter,
        })?;

    let rest: String = chars.collect();
    let (accidental, octave_text) = split_accidentals(&rest);
    let octave: i32 = octave_text.parse().map_err(|_| SynthError::BadOctave {
        name: name.to_owned(),
        octave: octave_text.to_owned(),
    })?;

    let midi = (octave + 1) * 12 + semitone + accidental;
    if !(MIDI_RANGE.0 as i32..=MIDI_RANGE.1 as i32).contains(&midi) {
        return Err(SynthError::NoteOutOfRange {
            name: name.to_owned(),
            midi,
        });
    }
    Ok(midi as f32)
}

fn split_accidentals(rest: &str) -> (i32, &str) {
    let mut offset = 0;
    let mut end = 0;
    for (i, c) in rest.char_indices() {
        match c {
            '#' | 's' => offset += 1,
            'b' | 'f' => offset -= 1,
            _ => break,
        }
        end = i + c.len_utf8();
    }
    (offset, &rest[end..])
}

#[inline]
pub fn midi_to_freq(midi: f32) -> f32 {
    A4_HZ * ((midi - A4_MIDI) / 12.0).exp2()
}

/// Sharps only: a pitch class has no key signature, and one spelling keeps two reports agreeing.
const SHARP_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

pub(crate) fn pitch_class_name(class: u32) -> &'static str {
    SHARP_NAMES[(class % 12) as usize]
}

pub(crate) fn midi_to_name(midi: f32) -> String {
    let whole = midi.round().clamp(MIDI_RANGE.0, MIDI_RANGE.1) as i32;
    format!(
        "{}{}",
        pitch_class_name(whole.unsigned_abs()),
        whole.div_euclid(12) - 1
    )
}
