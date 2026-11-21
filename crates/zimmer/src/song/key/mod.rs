//! The key a song is in: a tonic, a mode, and the arithmetic both buy.
//!
//! Every pitch in a song used to be absolute, which means the document knew
//! *which notes are played* and nothing at all about *which notes belong
//! together*. Two things follow from that, and the second is the larger one.
//!
//! **A wrong note was writable.** An agent writing a melody in D minor carries
//! the key entirely in its head, across hundreds of note objects, and a single
//! `F#` where an `F` belonged is a sour note that no validation catches and no
//! bake report measures. Written as a degree it is not a note that can be got
//! wrong: `3` in D minor is F, every time, because the document says so once.
//!
//! **A lift was not expressible.** An arrangement's `transpose` is chromatic —
//! every note by the same semitones — which moves the music to a different key
//! wholesale. That is exactly right for an octave double and it is *not* what
//! anybody means by "lift the last chorus". The gesture people mean is up a
//! step **within the key**, and it cannot even be stated until the document
//! says what the key is. See [`Key::shift`].
//!
//! ## The decisions, stated because getting one wrong is silent
//!
//! **Degrees count from one.** `1` is the tonic, `5` is the dominant, `8` is
//! the octave above the tonic. There is no zeroth degree and it is refused
//! rather than read as the tonic, because a writer who assumed zero-based
//! would otherwise get a whole part a step flat and nothing would say so.
//! Degrees past `7` keep climbing — `9` is the ninth — so a line that reaches
//! over the octave does not have to change `oct` mid-phrase.
//!
//! **`oct` places the tonic, and degrees stack upward from it.** `1` in
//! `D minor` at `oct: 4` is D4, and `7` is C5 rather than the C4 that sits
//! below it. The alternative — spelling the resulting note and putting *that*
//! in octave 4 — makes an ascending run of degrees jump back down partway
//! through, which is not what "up the scale" means anywhere. It is also the
//! rule [`Chord::oct`](super::Chord::oct) already follows for a chord's root.
//!
//! **The modes are the seven rotations of the diatonic collection**, and
//! nothing else: `major`/`ionian`, `dorian`, `phrygian`, `lydian`,
//! `mixolydian`, `minor`/`aeolian`, `locrian`. `major` and `minor` are the
//! names people actually say for two of them, and are spellings rather than
//! extra modes. The set is closed for the reason the chord table is closed —
//! a key that quietly means something other than what was written is worse
//! than the absolute notes it replaced. Harmonic and melodic minor are
//! deliberately out: they are not rotations of anything, they differ going up
//! from going down, and the raised seventh that motivates them is already
//! writable as `"#7"` — one character, against a second table with a caveat
//! attached.
//!
//! **Alterations are accidentals in front of the number** — `"b3"`, `"#4"`,
//! `"bb7"` — stacking the way they do in a note name, spelled `#` and `b`
//! only, the same narrowing [`chord::name`](super::chord::name) makes and for
//! the same reason. Without them every borrowed chord and every leading tone
//! in a minor key would be forced back to absolute names, which is most of the
//! interesting music.
//!
//! **Absolute names stay legal and unchanged**, in a song with a key as much
//! as in one without. A deliberate accidental is a real thing and has to stay
//! writable, and a document that already reads well as `"C#4"` has no reason
//! to be rewritten.
//!
//! ## Naming a chord by degree
//!
//! Not built here, and the shape it would take is worth leaving written down:
//! a [`Voicing`](super::Voicing) variant carrying a roman numeral, resolved
//! through [`Key::degree_midi`] for its root and through the existing quality
//! table for its stack. Nothing in this module would need to change for it.

pub(crate) mod degree;

use super::chord::name::split_root;
use crate::error::SynthError;
use crate::note::DIATONIC;

pub use degree::{Degree, DegreeNote};

/// Which rotation of the diatonic collection a key's scale is.
///
/// Closed, and written in rotation order — the discriminant *is* how far
/// around the collection the mode starts, which is the only thing the
/// arithmetic below needs from it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// The major scale; `"major"` spells this one too.
    Ionian,
    /// Minor with a raised sixth.
    Dorian,
    /// Minor with a flattened second.
    Phrygian,
    /// Major with a raised fourth.
    Lydian,
    /// Major with a flattened seventh.
    Mixolydian,
    /// The natural minor scale; `"minor"` spells this one too.
    Aeolian,
    /// Minor with a flattened second and a diminished fifth.
    Locrian,
}

impl Mode {
    /// Where this mode's scale starts within the diatonic collection.
    fn rotation(self) -> usize {
        self as usize
    }
}

/// Every spelling a mode answers to — the mode word alone, since the tonic in
/// front of it is read separately.
///
/// `major` and `minor` are the words people say for two of these, so they are
/// listed as spellings rather than as modes of their own: `"D minor"` and
/// `"D aeolian"` are one key and produce one set of notes.
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

/// How many steps a scale has before it starts again an octave up.
const STEPS: i32 = 7;

/// The key a song is in: which note is home, and which scale is built on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Key {
    /// The tonic's pitch class, `0` for C — the note degree `1` names.
    pub tonic: i32,
    /// Which scale is built on that tonic.
    pub mode: Mode,
}

impl Key {
    /// Reads a key — `"D minor"`, `"F# lydian"`, `"Bb major"` — or refuses it.
    ///
    /// The tonic is spelled the way a chord root is, through the same function:
    /// an **upper case** letter `A`–`G` with any number of `#` or `b` after it.
    /// The mode is one word from the table above, in any case, and it is
    /// **required** — a key that does not say which mode it is has not said
    /// what its third is, which is the one question this notation exists to
    /// answer.
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

    /// How far above the tonic the `step`-th step of this scale sits, in
    /// semitones, counting the tonic itself as step **0**.
    ///
    /// Zero-based here and one-based in the document on purpose: a degree is
    /// what a musician writes, a step is what the arithmetic counts, and the
    /// conversion happens once, at the edge, in [`Degree::read`]. Defined for
    /// every `i32`, so a step below the tonic or several octaves above it is
    /// the same expression rather than a special case.
    pub fn step_semitones(&self, step: i32) -> i32 {
        let rotation = self.mode.rotation();
        let within = step.rem_euclid(STEPS) as usize;
        let interval =
            (DIATONIC[(rotation + within) % DIATONIC.len()] - DIATONIC[rotation]).rem_euclid(12);
        interval + 12 * step.div_euclid(STEPS)
    }

    /// The MIDI number of `degree` — one-based, carrying `alteration`
    /// semitones of accidental — when the tonic sits in octave `oct`.
    pub fn degree_midi(&self, degree: i32, alteration: i32, oct: i32) -> i32 {
        (oct + 1) * 12 + self.tonic + self.step_semitones(degree - 1) + alteration
    }

    /// `midi` moved `steps` steps **within this key** — the diatonic
    /// transpose, which is what a chorus lifting actually does.
    ///
    /// A pitch that is in the key moves to the scale note `steps` away, so the
    /// music stays in the key it was written in rather than moving to another
    /// one. A pitch that is **not** in the key — a deliberate accidental, a
    /// chromatic passing note, the pitch a drum happens to be tuned to — keeps
    /// its distance above the scale note below it, and so comes out as the
    /// same kind of accidental in its new position. Refusing those instead
    /// would make a legal lift depend on whether some other track happens to
    /// be chromatic, which is not a relationship anybody wants to maintain.
    ///
    /// Fractional MIDI survives it: a microtonal pitch moves by whatever its
    /// whole part moved by, and keeps its fraction.
    ///
    /// **A shift of no steps is exactly the identity**, for a pitch in the key
    /// and for one outside it alike — the decomposition and the recomposition
    /// are inverses. That is load-bearing rather than incidental:
    /// [`played_pitch`](crate::song::Layer::played_pitch) runs every note of a
    /// keyed song through here rather than branching around an entry that
    /// lifts by nothing.
    pub fn shift(&self, midi: f32, steps: i32) -> f32 {
        let whole = midi.floor();
        let fraction = midi - whole;
        let above = whole as i32 - self.tonic;
        let octaves = above.div_euclid(12);
        let inside = above.rem_euclid(12);
        // The highest step at or below the note: whatever is above that is the
        // accidental, and it rides along untouched.
        let step = (0..STEPS)
            .rev()
            .find(|step| self.step_semitones(*step) <= inside)
            .unwrap_or(0);
        let alteration = inside - self.step_semitones(step);
        let moved = self.step_semitones(step + octaves * STEPS + steps) + alteration;
        (self.tonic + moved) as f32 + fraction
    }

    /// Whether `midi`, rounded to the nearest semitone, is a note of this
    /// key's scale in any octave.
    pub fn contains(&self, midi: f32) -> bool {
        let inside = (midi.round() as i32 - self.tonic).rem_euclid(12);
        (0..STEPS).any(|step| self.step_semitones(step) == inside)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One octave of a key's scale as MIDI, with its tonic at `oct`.
    fn scale(text: &str, oct: i32) -> Vec<i32> {
        let key = Key::parse(text).expect("a key from the table");
        (1..=8)
            .map(|degree| key.degree_midi(degree, 0, oct))
            .collect()
    }

    /// Two keys spelled out as the notes everyone already knows them by.
    #[test]
    fn a_key_names_the_scale_everyone_means_by_it() {
        // C4 D4 E4 F4 G4 A4 B4 C5.
        assert_eq!(scale("C major", 4), vec![60, 62, 64, 65, 67, 69, 71, 72]);
        // D4 E4 F4 G4 A4 Bb4 C5 D5 — the F natural and the C natural, which
        // are the whole reason a document says `D minor` once instead of
        // getting them right two hundred times.
        assert_eq!(scale("D minor", 4), vec![62, 64, 65, 67, 69, 70, 72, 74]);
        // The two words that are spellings rather than modes of their own.
        assert_eq!(scale("D aeolian", 4), scale("D minor", 4));
        assert_eq!(scale("C ionian", 4), scale("C major", 4));
    }

    /// Each church mode, by the one degree that tells it from its parent.
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

    /// Degrees count from one, keep climbing past the octave, and take their
    /// octave from where the *tonic* was placed.
    #[test]
    fn degrees_count_from_the_tonic_upward() {
        let key = Key::parse("D minor").expect("a key");
        assert_eq!(key.degree_midi(1, 0, 4), 62, "degree 1 is the tonic itself");
        assert_eq!(key.degree_midi(9, 0, 4), 76, "the ninth is over the octave");
        // The leading tone, which is why alterations are not optional.
        assert_eq!(key.degree_midi(7, 1, 4), 73);
        assert_eq!(key.degree_midi(3, -1, 4), 64, "a flattened third");
    }

    /// A lift moves within the key, which is the whole point of it.
    #[test]
    fn a_diatonic_shift_stays_in_the_key() {
        let key = Key::parse("D minor").expect("a key");
        // D4 E4 F4 up one step is E4 F4 G4 — two whole tones and a semitone
        // moved by different amounts, which one chromatic number cannot do.
        let up: Vec<f32> = [62.0, 64.0, 65.0]
            .iter()
            .map(|midi| key.shift(*midi, 1))
            .collect();
        assert_eq!(up, vec![64.0, 65.0, 67.0]);
        // And back down again, and by a whole octave's worth of steps.
        assert_eq!(key.shift(65.0, -1), 64.0);
        assert_eq!(key.shift(62.0, 7), 74.0);
    }

    /// An accidental keeps its distance above the scale note under it rather
    /// than being snapped into the key or refused.
    #[test]
    fn a_note_outside_the_key_keeps_its_accidental() {
        let key = Key::parse("D minor").expect("a key");
        // C#5, the leading tone, moves to D#5 — still a sharpened scale note.
        assert_eq!(key.shift(73.0, 1), 75.0);
        // A microtonal pitch keeps its fraction through the move.
        assert_eq!(key.shift(62.5, 1), 64.5);
    }

    /// A lift of no steps leaves every pitch exactly where it was.
    ///
    /// Stated as its own test because something depends on it: an arrangement
    /// entry does not branch around a `transpose_degrees` of zero, so if the
    /// decomposition here ever stopped being reversible — snapping an
    /// accidental into the key, say — every note of every keyed song would
    /// move and no other test would be looking.
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

    /// The grammar is closed: a key is a tonic and a mode, both spelled the
    /// one way.
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
        // The mode word is a word, so its case is not load-bearing.
        assert_eq!(
            Key::parse("D Minor").expect("a key"),
            Key::parse("D minor").expect("a key")
        );
    }
}
