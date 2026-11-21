//! Notes, pitch, and the per-note render options.
//!
//! A patch is *playable as a note*: `(pitch, velocity, duration) in → buffer out`.
//! This module owns the pitch half of that contract — the note **name ↔ MIDI ↔ Hz**
//! conversions — plus [`NoteOpts`], the small bundle carrying the other two (and the
//! seed every stochastic source draws from).
//!
//! Pitch is **equal temperament** anchored at A4 = 440 Hz (MIDI 69):
//! `f = 440 × 2^((midi − 69) / 12)`. MIDI is kept as `f32` throughout, so a
//! fractional note number is a legal microtonal pitch and so the LFO can bend pitch
//! in fractions of a semitone.
//!
//! Note names are the usual scientific pitch notation: a letter `A`–`G`, any
//! number of accidentals (`#`/`s` sharp, `b`/`f` flat), then the octave
//! (`C4` = middle C = MIDI 60, `C-1` = MIDI 0). Parsing lives here because a
//! song's notes reuse it.

use crate::error::SynthError;

/// The MIDI pitch axis, lowest and highest — `C-1` to `G9`.
///
/// Shared rather than duplicated because it is a boundary two different things
/// need: a note *name* outside it is refused here, and a transposed note pushed
/// outside it is clamped back into it by the song renderer. Two copies of the
/// range would be two places to disagree about what a pitch is.
pub(crate) const MIDI_RANGE: (f32, f32) = (0.0, 127.0);

/// MIDI note number of the tuning reference, A4.
const A4_MIDI: f32 = 69.0;
/// Concert pitch of A4, in Hz.
const A4_HZ: f32 = 440.0;

/// Semitone offset of each natural note within its octave (C = 0).
const NATURALS: [(char, i32); 7] = [
    ('c', 0),
    ('d', 2),
    ('e', 4),
    ('f', 5),
    ('g', 7),
    ('a', 9),
    ('b', 11),
];

/// The diatonic collection, as semitones above the root of its major mode.
///
/// One array rather than two, because two things count against it and they
/// must not be able to disagree: [`song::key`](crate::song::key) rotates it to
/// get every mode's scale, and [`survey`](crate::survey) matches a piece's
/// pitch classes against it to name the collection they *are*. It lives here
/// because this module owns the pitch axis those two share.
pub(crate) const DIATONIC: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];

/// Where a note starts, when it does not start on its own pitch.
///
/// A **glide** is the one thing about a note that is not a property of the
/// note: it says *this pitch is arrived at*, from somewhere the note itself
/// does not name. What that somewhere is belongs to whoever is playing a line
/// — the song renderer works it out from the note before — and by the time it
/// reaches here it is a distance and a time, which is all the frequency track
/// needs.
///
/// Semitones rather than a start pitch, because pitch is logarithmic and every
/// other thing that bends a note here is in semitones too: a glide, a vibrato
/// and a pitch envelope are three terms of one sum, and a term in Hz could not
/// be added to the other two.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glide {
    /// How far from its own pitch the note starts, in semitones. Positive
    /// starts above and falls onto the note; a slide *up* into one is
    /// negative.
    pub semitones: f32,
    /// How long the slide takes. The note is on the pitch it was written at
    /// from there until it is released.
    pub seconds: f32,
}

/// How one note is rendered: how long the key is held, how hard it is struck, and
/// the seed the stochastic sources (noise, Karplus excitation) draw from.
///
/// `duration` is the **gate** length — the amp envelope's release rings out *after*
/// it, so the rendered buffer is longer than `duration` (see
/// [`crate::render_note`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NoteOpts {
    /// Gate length in seconds — how long the note is held before release.
    pub duration: f32,
    /// Striking force in `0..=1`, scaling the note's peak amplitude.
    pub velocity: f32,
    /// How far this strike's *brightness* sits from its level, in the units
    /// `velocity` is in.
    ///
    /// Added to `velocity` for the two routings that read velocity as effort —
    /// a filter's `vel_octaves` and two-operator FM's `vel_index` — and nowhere
    /// else, so a note can be played a shade brighter or duller without being
    /// played louder. `0.0` means the two are one number, which is what every
    /// note meant before a performance could scatter them apart (see
    /// [`Humanize::timbre`](field@crate::song::Humanize::timbre)).
    pub timbre: f32,
    /// Where this note slides in from, or nothing — struck on its own pitch,
    /// which is what every note meant before a line could be played legato.
    ///
    /// It sits with the performance rather than with the patch, beside the
    /// velocity and the gate, because that is what it is: a
    /// [`PitchEnv`](crate::patch::PitchEnv) is a shape the *instrument* puts
    /// on every note it plays, and a glide is where the hand happened to be
    /// before this one.
    pub glide: Option<Glide>,
    /// Seed for every stochastic source; the same seed replays the same noise.
    pub seed: u64,
}

impl Default for NoteOpts {
    /// A half-second note at a comfortable velocity, seed 0 — sane for a one-shot.
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

/// Parses a note name (`"C#4"`, `"Bb3"`, `"a-1"`) to its MIDI number.
///
/// Case-insensitive; accidentals stack, so `"Cbb4"` is 58.
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

/// Splits the accidental run off the front of `rest`, returning its total
/// semitone offset and the remaining (octave) text.
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

/// Equal-temperament frequency of a (possibly fractional) MIDI note, in Hz.
#[inline]
pub fn midi_to_freq(midi: f32) -> f32 {
    A4_HZ * ((midi - A4_MIDI) / 12.0).exp2()
}

/// The twelve pitch classes, spelled with sharps.
///
/// One spelling rather than the enharmonic pair, because a pitch class is a
/// number here and a number carries no key signature to be spelled in — `A#`
/// and `Bb` are the same class, and picking one keeps two reports of the same
/// music from disagreeing about what it is made of.
const SHARP_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

/// Names a pitch class — `0` is C, `11` is B — wrapping anything above.
pub(crate) fn pitch_class_name(class: u32) -> &'static str {
    SHARP_NAMES[(class % 12) as usize]
}

/// Names a MIDI number the way a score does: `60` is `"C4"`.
///
/// The inverse of [`parse_note`] for whole numbers, and rounded to the nearest
/// semitone for the fractional ones a microtonal pitch or a transpose can
/// produce — a report says which note this *is*, and "C4 and a bit" is not a
/// note anyone goes looking for.
pub(crate) fn midi_to_name(midi: f32) -> String {
    let whole = midi.round().clamp(MIDI_RANGE.0, MIDI_RANGE.1) as i32;
    format!(
        "{}{}",
        pitch_class_name(whole.unsigned_abs()),
        whole.div_euclid(12) - 1
    )
}
