//! The key a song is in: a tonic, a mode, and the arithmetic both buy.

pub(crate) mod degree;

use super::chord::name::split_root;
use crate::error::SynthError;
use crate::note::DIATONIC;

pub use degree::{Degree, DegreeNote};

/// In rotation order: `rotation` reads the discriminant, so the order is the arithmetic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Ionian,
    Dorian,
    Phrygian,
    Lydian,
    Mixolydian,
    Aeolian,
    Locrian,
}

impl Mode {
    fn rotation(self) -> usize {
        self as usize
    }
}

const SPELLINGS: [(&str, Mode); 9] = [
    ("major", Mode::Ionian),
    ("ionian", Mode::Ionian),
    ("dorian", Mode::Dorian),
    ("phrygian", Mode::Phrygian),
    ("lydian", Mode::Lydian),
    ("mixolydian", Mode::Mixolydian),
    ("minor", Mode::Aeolian),
    ("aeolian", Mode::Aeolian),
    ("locrian", Mode::Locrian),
];

const STEPS: i32 = 7;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Key {
    pub tonic: i32,
    pub mode: Mode,
}

impl Key {
    pub fn parse(text: &str) -> Result<Self, SynthError> {
        let bad = || SynthError::BadKey {
            key: text.to_owned(),
        };
        let (tonic_text, mode_text) = text
            .trim()
            .split_once(char::is_whitespace)
            .ok_or_else(bad)?;
        let (tonic, rest) = split_root(tonic_text).ok_or_else(bad)?;
        if !rest.is_empty() {
            return Err(bad());
        }
        let mode = SPELLINGS
            .iter()
            .find(|(spelled, _)| spelled.eq_ignore_ascii_case(mode_text.trim()))
            .map(|(_, mode)| *mode)
            .ok_or_else(bad)?;
        Ok(Self { tonic, mode })
    }

    /// Semitones above the tonic of zero-based `step`; `degree_midi` converts the page's
    /// one-based degrees.
    pub fn step_semitones(&self, step: i32) -> i32 {
        let rotation = self.mode.rotation();
        let within = step.rem_euclid(STEPS) as usize;
        let interval =
            (DIATONIC[(rotation + within) % DIATONIC.len()] - DIATONIC[rotation]).rem_euclid(12);
        interval + 12 * step.div_euclid(STEPS)
    }

    pub fn degree_midi(&self, degree: i32, alteration: i32, oct: i32) -> i32 {
        (oct + 1) * 12 + self.tonic + self.step_semitones(degree - 1) + alteration
    }

    pub fn shift(&self, midi: f32, steps: i32) -> f32 {
        let whole = midi.floor();
        let fraction = midi - whole;
        let above = whole as i32 - self.tonic;
        let octaves = above.div_euclid(12);
        let inside = above.rem_euclid(12);
        let step = (0..STEPS)
            .rev()
            .find(|step| self.step_semitones(*step) <= inside)
            .unwrap_or(0);
        let alteration = inside - self.step_semitones(step);
        let moved = self.step_semitones(step + octaves * STEPS + steps) + alteration;
        (self.tonic + moved) as f32 + fraction
    }

    pub fn contains(&self, midi: f32) -> bool {
        let inside = (midi.round() as i32 - self.tonic).rem_euclid(12);
        (0..STEPS).any(|step| self.step_semitones(step) == inside)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scale(text: &str, oct: i32) -> Vec<i32> {
        let key = Key::parse(text).expect("a key from the table");
        (1..=8)
            .map(|degree| key.degree_midi(degree, 0, oct))
            .collect()
    }

    #[test]
    fn a_key_names_the_scale_everyone_means_by_it() {
        assert_eq!(scale("C major", 4), vec![60, 62, 64, 65, 67, 69, 71, 72]);
        assert_eq!(scale("D minor", 4), vec![62, 64, 65, 67, 69, 70, 72, 74]);
        assert_eq!(scale("D aeolian", 4), scale("D minor", 4));
        assert_eq!(scale("C ionian", 4), scale("C major", 4));
    }

    #[test]
    fn the_modes_differ_where_their_names_say_they_do() {
        let degree = |text: &str, degree: i32| {
            Key::parse(text).expect("a key").degree_midi(degree, 0, 4) - 60
        };
        assert_eq!(degree("C dorian", 6), 9, "dorian raises the sixth");
        assert_eq!(degree("C phrygian", 2), 1, "phrygian flattens the second");
        assert_eq!(degree("C lydian", 4), 6, "lydian raises the fourth");
        assert_eq!(degree("C mixolydian", 7), 10, "mixolydian flattens the 7th");
        assert_eq!(degree("C locrian", 5), 6, "locrian diminishes the fifth");
    }

    #[test]
    fn degrees_count_from_the_tonic_upward() {
        let key = Key::parse("D minor").expect("a key");
        assert_eq!(key.degree_midi(1, 0, 4), 62, "degree 1 is the tonic itself");
        assert_eq!(key.degree_midi(9, 0, 4), 76, "the ninth is over the octave");
        // The leading tone, which is why alterations are not optional.
        assert_eq!(key.degree_midi(7, 1, 4), 73);
        assert_eq!(key.degree_midi(3, -1, 4), 64, "a flattened third");
    }

    #[test]
    fn a_diatonic_shift_stays_in_the_key() {
        let key = Key::parse("D minor").expect("a key");
        // D4 E4 F4 up one step is E4 F4 G4 — two whole tones and a semitone moved by different
        // amounts, which one chromatic number cannot do.
        let up: Vec<f32> = [62.0, 64.0, 65.0]
            .iter()
            .map(|midi| key.shift(*midi, 1))
            .collect();
        assert_eq!(up, vec![64.0, 65.0, 67.0]);
        assert_eq!(key.shift(65.0, -1), 64.0);
        assert_eq!(key.shift(62.0, 7), 74.0);
    }

    #[test]
    fn a_note_outside_the_key_keeps_its_accidental() {
        let key = Key::parse("D minor").expect("a key");
        assert_eq!(key.shift(73.0, 1), 75.0);
        assert_eq!(key.shift(62.5, 1), 64.5);
    }

    #[test]
    fn a_key_holds_its_seven_classes_in_every_octave() {
        let key = Key::parse("E major").expect("a key");
        let held: Vec<i32> = (0..12)
            .filter(|class| key.contains(*class as f32))
            .collect();
        assert_eq!(held, vec![1, 3, 4, 6, 8, 9, 11], "C# D# E F# G# A B");
        assert!(key.contains(40.0 + 24.0), "E an octave up");
        assert!(!key.contains(41.0), "F is not in E major");
    }

    #[test]
    fn a_shift_of_no_steps_is_the_identity() {
        for text in ["C major", "D minor", "F# lydian", "B locrian"] {
            let key = Key::parse(text).expect("a key");
            for midi in [0.0, 21.0, 60.0, 62.5, 73.0, 100.75, 127.0] {
                assert_eq!(key.shift(midi, 0), midi, "{text} moved {midi}");
            }
        }
    }

    #[test]
    fn anything_that_is_not_a_tonic_and_a_mode_is_refused() {
        for text in [
            "",
            "D",
            "d minor",
            "H minor",
            "D harmonic minor",
            "D melodic",
            "Dminor",
            "D minr",
            "Ds minor",
            "minor",
        ] {
            assert!(Key::parse(text).is_err(), "`{text}` should not have parsed");
        }
        assert_eq!(
            Key::parse("D Minor").expect("a key"),
            Key::parse("D minor").expect("a key")
        );
    }
}
