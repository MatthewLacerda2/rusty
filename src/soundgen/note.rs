//! src/soundgen/note.rs — notes, pitch, and the per-note render options (#357).
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
//! Note names are the usual scientific pitch notation: a letter `A`–`G`, any number
//! of accidentals (`#`/`s` sharp, `b`/`f` flat), then the octave (`C4` = middle C =
//! MIDI 60, `C-1` = MIDI 0). Parsing lives here because the song leg reuses it.

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

/// How one note is rendered: how long the key is held, how hard it is struck, and
/// the seed the stochastic sources (noise, Karplus excitation) draw from.
///
/// `duration` is the **gate** length — the amp envelope's release rings out *after*
/// it, so the rendered buffer is longer than `duration` (see
/// [`crate::soundgen::core::render`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NoteOpts {
    /// Gate length in seconds — how long the note is held before release.
    pub duration: f32,
    /// Striking force in `0..=1`, scaling the note's peak amplitude.
    pub velocity: f32,
    /// Seed for every stochastic source; the same seed replays the same noise.
    pub seed: u64,
}

impl Default for NoteOpts {
    /// A half-second note at a comfortable velocity, seed 0 — sane for a one-shot.
    fn default() -> Self {
        Self {
            duration: 0.5,
            velocity: 0.8,
            seed: 0,
        }
    }
}

/// Parse a note name (`"C#4"`, `"Bb3"`, `"a-1"`) to its MIDI number.
///
/// Case-insensitive; accidentals stack (`"Cbb4"` is 58). Returns an error message
/// the script surface shows verbatim.
pub fn parse_note(name: &str) -> Result<f32, String> {
    let text = name.trim().to_ascii_lowercase();
    let mut chars = text.chars();
    let letter = chars.next().ok_or_else(|| "empty note name".to_string())?;
    let semitone = NATURALS
        .iter()
        .find(|(c, _)| *c == letter)
        .map(|(_, s)| *s)
        .ok_or_else(|| format!("note `{name}`: expected a letter A–G, got `{letter}`"))?;

    let rest: String = chars.collect();
    let (accidental, octave_text) = split_accidentals(&rest);
    let octave: i32 = octave_text
        .parse()
        .map_err(|_| format!("note `{name}`: `{octave_text}` is not an octave number"))?;

    let midi = (octave + 1) * 12 + semitone + accidental;
    if !(0..=127).contains(&midi) {
        return Err(format!("note `{name}`: MIDI {midi} is outside 0..=127"));
    }
    Ok(midi as f32)
}

/// Split the accidental run off the front of `rest`, returning its total semitone
/// offset and the remaining (octave) text.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn middle_c_and_concert_a_have_the_standard_numbers() {
        assert_eq!(parse_note("C4").unwrap(), 60.0);
        assert_eq!(parse_note("A4").unwrap(), 69.0);
        assert_eq!(parse_note("C-1").unwrap(), 0.0);
        assert_eq!(parse_note("G9").unwrap(), 127.0);
    }

    #[test]
    fn accidentals_are_case_insensitive_and_stack() {
        assert_eq!(parse_note("C#4").unwrap(), 61.0);
        assert_eq!(parse_note("db4").unwrap(), 61.0);
        assert_eq!(parse_note("cs4").unwrap(), 61.0);
        assert_eq!(parse_note("Bb3").unwrap(), 58.0);
        assert_eq!(parse_note("C##4").unwrap(), 62.0);
    }

    #[test]
    fn malformed_names_report_why() {
        assert!(parse_note("").is_err());
        assert!(parse_note("H4").is_err());
        assert!(parse_note("C").is_err());
        assert!(parse_note("Cx4").is_err());
        assert!(parse_note("C-2").is_err(), "MIDI -12 is out of range");
    }

    #[test]
    fn equal_temperament_anchors_at_440() {
        assert!((midi_to_freq(69.0) - 440.0).abs() < 1e-3);
        assert!(
            (midi_to_freq(81.0) - 880.0).abs() < 1e-3,
            "an octave up doubles"
        );
        assert!(
            (midi_to_freq(57.0) - 220.0).abs() < 1e-3,
            "an octave down halves"
        );
        // Middle C, the other number everyone knows.
        assert!((midi_to_freq(60.0) - 261.6256).abs() < 1e-2);
    }

    #[test]
    fn default_opts_describe_a_short_one_shot() {
        let o = NoteOpts::default();
        assert_eq!(o.duration, 0.5);
        assert_eq!(o.seed, 0);
        assert!(o.velocity > 0.0 && o.velocity <= 1.0);
    }
}
