//! Pitch read as a *set* rather than as a sequence: how high and low a piece reaches, and which
//! twelve-tone classes it draws from.

use std::fmt;

use crate::note::{midi_to_name, pitch_class_name, DIATONIC};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Register {
    pub low: f32,
    pub high: f32,
}

impl Register {
    pub fn at(midi: f32) -> Self {
        Self {
            low: midi,
            high: midi,
        }
    }

    #[must_use]
    pub fn with(self, midi: f32) -> Self {
        Self {
            low: self.low.min(midi),
            high: self.high.max(midi),
        }
    }

    #[must_use]
    pub fn union(self, other: Self) -> Self {
        self.with(other.low).with(other.high)
    }

    pub fn semitones(self) -> f32 {
        self.high - self.low
    }
}

impl fmt::Display for Register {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (low, high) = (midi_to_name(self.low), midi_to_name(self.high));
        if low == high {
            write!(f, "{low}")
        } else {
            write!(f, "{low}-{high}")
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PitchClasses(u16);

impl PitchClasses {
    pub fn add(&mut self, midi: f32) {
        let class = midi.round().rem_euclid(12.0) as u32;
        self.0 |= 1 << class;
    }

    pub fn classes(self) -> impl Iterator<Item = u32> {
        (0..12).filter(move |class| self.0 & (1 << class) != 0)
    }

    pub fn count(self) -> u32 {
        self.0.count_ones()
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

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

    #[test]
    fn seven_classes_are_named_as_a_collection_only_when_they_are_one() {
        let of = |notes: &[f32]| {
            let mut classes = PitchClasses::default();
            for midi in notes {
                classes.add(*midi);
            }
            classes
        };
        let major = of(&[60.0, 62.0, 64.0, 65.0, 67.0, 69.0, 71.0]);
        assert_eq!(major.diatonic_root(), Some("C"));
        let minor = of(&[69.0, 71.0, 72.0, 74.0, 76.0, 77.0, 79.0]);
        assert_eq!(minor.diatonic_root(), Some("C"));
        assert_eq!(
            of(&[69.0, 71.0, 72.0, 74.0, 76.0, 77.0, 80.0]).diatonic_root(),
            None
        );
        assert_eq!(of(&[60.0, 62.0, 64.0]).diatonic_root(), None);
    }
}
