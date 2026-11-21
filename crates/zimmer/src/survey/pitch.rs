//! Pitch read as a *set* rather than as a sequence: how high and low a piece
//! reaches, and which twelve-tone classes it draws from.
//!
//! Both are facts about the notes already written down, arrived at by counting
//! them. Neither is an analysis: nothing here decides a key, a tonic or a
//! chord, because those are inferences about intent and a document that lists
//! seven pitch classes has not stated one.

use std::fmt;

use crate::note::{DIATONIC, midi_to_name, pitch_class_name};

/// The lowest and highest pitch some notes reach, as MIDI numbers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Register {
    /// The lowest note, in MIDI.
    pub low: f32,
    /// The highest note, in MIDI.
    pub high: f32,
}

impl Register {
    /// The register `midi` alone occupies.
    pub fn at(midi: f32) -> Self {
        Self {
            low: midi,
            high: midi,
        }
    }

    /// This register widened to hold `midi` as well.
    #[must_use]
    pub fn with(self, midi: f32) -> Self {
        Self {
            low: self.low.min(midi),
            high: self.high.max(midi),
        }
    }

    /// The register covering both, for a report that spans several pieces.
    #[must_use]
    pub fn union(self, other: Self) -> Self {
        self.with(other.low).with(other.high)
    }

    /// How wide it is, in semitones.
    pub fn semitones(self) -> f32 {
        self.high - self.low
    }
}

impl fmt::Display for Register {
    /// `E1-A5`, and a single note as itself rather than as a range of nothing.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (low, high) = (midi_to_name(self.low), midi_to_name(self.high));
        if low == high {
            write!(f, "{low}")
        } else {
            write!(f, "{low}-{high}")
        }
    }
}

/// Which of the twelve pitch classes a set of notes uses, as a bit per class.
///
/// A set rather than a count of each: how *often* a note is played is a
/// property of the music, and how many different ones there are is a property
/// of the collection it was written in, which is the question this answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PitchClasses(u16);

impl PitchClasses {
    /// Adds the class `midi` belongs to, rounded to the nearest semitone.
    pub fn add(&mut self, midi: f32) {
        let class = midi.round().rem_euclid(12.0) as u32;
        self.0 |= 1 << class;
    }

    /// Every class in the set, low to high.
    pub fn classes(self) -> impl Iterator<Item = u32> {
        (0..12).filter(move |class| self.0 & (1 << class) != 0)
    }

    /// How many distinct classes there are — `0` when nothing was added, `12`
    /// for a fully chromatic piece.
    pub fn count(self) -> u32 {
        self.0.count_ones()
    }

    /// Whether any note was ever added.
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The root of the diatonic collection this set *is*, when it is exactly
    /// one of the twelve.
    ///
    /// Arithmetic, not analysis: `{C D E F G A B}` is the diatonic collection
    /// of C whichever of its modes the music is actually in, and naming the
    /// mode would mean picking a tonic the document never states. A set that
    /// is not a diatonic collection is simply not named.
    pub fn diatonic_root(self) -> Option<&'static str> {
        (0..12u32)
            .find(|root| {
                let mask: u16 = DIATONIC
                    .iter()
                    .map(|step| 1u16 << ((root + *step as u32) % 12))
                    .sum();
                mask == self.0
            })
            .map(pitch_class_name)
    }
}

impl fmt::Display for PitchClasses {
    /// The classes in order, `C D E F G A B`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<&str> = self.classes().map(pitch_class_name).collect();
        write!(f, "{}", names.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_register_is_named_the_way_a_score_names_its_notes() {
        let register = Register::at(40.0).with(81.0).with(28.0);
        assert_eq!(register.to_string(), "E1-A5");
        assert_eq!(register.semitones(), 53.0);
        assert_eq!(Register::at(60.0).to_string(), "C4");
    }

    #[test]
    fn the_classes_are_the_distinct_notes_whatever_octave_they_were_played_in() {
        let mut classes = PitchClasses::default();
        for midi in [60.0, 62.0, 64.0, 72.0, 74.0] {
            classes.add(midi);
        }
        assert_eq!(classes.count(), 3);
        assert_eq!(classes.to_string(), "C D E");
        assert!(!classes.is_empty());
    }

    /// The collection is named when the set is exactly one, and left unnamed
    /// otherwise — never guessed at.
    #[test]
    fn seven_classes_are_named_as_a_collection_only_when_they_are_one() {
        let of = |notes: &[f32]| {
            let mut classes = PitchClasses::default();
            for midi in notes {
                classes.add(*midi);
            }
            classes
        };
        // C D E F G A B.
        let major = of(&[60.0, 62.0, 64.0, 65.0, 67.0, 69.0, 71.0]);
        assert_eq!(major.diatonic_root(), Some("C"));
        // The same seven classes starting on A — one collection, not two.
        let minor = of(&[69.0, 71.0, 72.0, 74.0, 76.0, 77.0, 79.0]);
        assert_eq!(minor.diatonic_root(), Some("C"));
        // Harmonic minor is seven notes and is not a diatonic collection.
        assert_eq!(
            of(&[69.0, 71.0, 72.0, 74.0, 76.0, 77.0, 80.0]).diatonic_root(),
            None
        );
        assert_eq!(of(&[60.0, 62.0, 64.0]).diatonic_root(), None);
    }
}
