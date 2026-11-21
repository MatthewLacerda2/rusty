//! A note written as a degree of the song's key rather than as a pitch.
//!
//! `{ "track": "lead", "degree": 5, "oct": 4, "start": 0, "dur": 1 }` is the
//! dominant of whatever key the song declared, and it stays the dominant when
//! the key changes. The reasoning, the numbering and the alteration grammar
//! are all in the [module above](super); this file is the document form and
//! the one step from it to a pitch.
//!
//! A degree in a song with **no key** is refused rather than assumed into C:
//! guessing a key is exactly the analysis this crate does not do, and a whole
//! part rendered a fourth away from where it was meant is not a mistake
//! anybody finds by reading.

use serde::{Deserialize, Serialize};

use super::Key;
use crate::error::SynthError;
use crate::note::MIDI_RANGE;
use crate::song::{Articulation, Note, Pitch, one};

/// Where the tonic sits when the document does not say.
///
/// Octave 4 puts degree `1` on middle C in a song in C, which is the register
/// a written line sits in — a degree is most often a melody, where
/// [`Chord::oct`](crate::song::Chord::oct)'s 3 is a chord being voiced under
/// one.
const DEFAULT_OCT: i32 = 4;

/// One note of the key's scale: which track plays it, which degree, when, for
/// how long, how hard.
///
/// Every field but `degree` and `oct` means exactly what it means on a
/// [`Note`], because that is what it becomes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DegreeNote {
    /// The [`Track::name`](crate::song::Track::name) that plays it.
    pub track: String,
    /// Which degree of the song's key, counting the tonic as `1`.
    pub degree: Degree,
    /// Which octave the **tonic** sits in, so degrees climb away from it:
    /// `1` in `D minor` at `4` is D4 and `7` is C5. Absent means octave 4,
    /// which puts the tonic of a major key at or near middle C.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oct: Option<i32>,
    /// Onset in beats from the start of its pattern.
    pub start: f32,
    /// Gate length in beats. The patch's release rings out after it.
    pub dur: f32,
    /// Velocity in `0..=1`, scaling this note's peak amplitude.
    #[serde(default = "one")]
    pub vel: f32,
    /// How it is played — see [`Note::articulation`]. A degree is one note
    /// written a different way, so it carries the field for the same reason a
    /// note does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub articulation: Option<Articulation>,
}

/// Which degree, and whether an accidental is on it.
///
/// Untagged, and `Plain` first: a JSON number can only be a bare degree and a
/// JSON string can only be an altered one, so the two never race — the trick
/// [`Pitch`] already uses.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Degree {
    /// A degree of the scale as written — `5` is the dominant.
    Plain(i32),
    /// A degree with accidentals in front of it — `"b3"`, `"#4"`, `"bb7"`.
    Altered(String),
}

impl Degree {
    /// The degree and how many semitones of accidental sit on it, or a refusal.
    ///
    /// The one place a written degree becomes a number the arithmetic counts,
    /// which is where the one-based document meets the zero-based
    /// [`Key::step_semitones`].
    pub fn read(&self) -> Result<(i32, i32), SynthError> {
        let bad = || SynthError::BadDegree {
            degree: self.written(),
        };
        let (degree, alteration) = match self {
            Self::Plain(degree) => (*degree, 0),
            Self::Altered(text) => {
                let (alteration, rest) = split_accidentals(text);
                (rest.parse().map_err(|_| bad())?, alteration)
            }
        };
        // Counting from one is the rule, so zero is the reading a zero-based
        // writer would have got away with — and a part a step flat throughout
        // is not something anybody notices by reading it back.
        if degree < 1 {
            return Err(bad());
        }
        Ok((degree, alteration))
    }

    /// This degree as the document spells it, for an error to quote back.
    fn written(&self) -> String {
        match self {
            Self::Plain(degree) => degree.to_string(),
            Self::Altered(text) => text.clone(),
        }
    }
}

/// Splits the accidental run off the front, returning its total semitone
/// offset and the remaining (degree) text.
///
/// `#` and `b` only — the same narrowing the chord grammar makes, so one
/// spelling of an accidental works everywhere in a song document.
fn split_accidentals(text: &str) -> (i32, &str) {
    let mut offset = 0;
    let mut end = 0;
    for (index, character) in text.char_indices() {
        match character {
            '#' => offset += 1,
            'b' => offset -= 1,
            _ => break,
        }
        end = index + character.len_utf8();
    }
    (offset, &text[end..])
}

impl DegreeNote {
    /// Appends the note this degree sounds as to `out`, or says why it cannot.
    ///
    /// Appends rather than returns for the reason a chord does: a pattern's
    /// voices are built into one allocation, in document order, so the
    /// ordinals the renderer hands out are the ones the page implies.
    pub(crate) fn voice_into(
        &self,
        key: Option<&Key>,
        out: &mut Vec<Note>,
    ) -> Result<(), SynthError> {
        let key = key.ok_or_else(|| SynthError::DegreeWithoutKey {
            track: self.track.clone(),
            start: self.start,
        })?;
        let (degree, alteration) = self.degree.read()?;
        let oct = self.oct.unwrap_or(DEFAULT_OCT);
        let midi = key.degree_midi(degree, alteration, oct);
        if !(MIDI_RANGE.0 as i32..=MIDI_RANGE.1 as i32).contains(&midi) {
            return Err(SynthError::DegreeOutOfRange {
                degree: self.degree.written(),
                oct,
                midi,
            });
        }
        out.push(Note {
            track: self.track.clone(),
            note: Pitch::Midi(midi as f32),
            start: self.start,
            dur: self.dur,
            vel: self.vel,
            articulation: self.articulation,
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A degree on the `lead` track, at the octave the document gave it.
    fn degree(written: Degree, oct: Option<i32>) -> DegreeNote {
        DegreeNote {
            track: "lead".to_owned(),
            degree: written,
            oct,
            start: 0.0,
            dur: 1.0,
            vel: 0.8,
            articulation: None,
        }
    }

    /// The MIDI number a degree comes out as in `key`.
    fn voiced(note: &DegreeNote, key: Option<&str>) -> Result<f32, SynthError> {
        let key = key.map(|text| Key::parse(text).expect("a key"));
        let mut out = Vec::new();
        note.voice_into(key.as_ref(), &mut out)?;
        out[0].note.to_midi()
    }

    /// The numbering rule and the alteration grammar, as pitches.
    #[test]
    fn a_degree_is_the_scale_note_it_names() {
        let plain = |number| degree(Degree::Plain(number), Some(4));
        let altered = |text: &str| degree(Degree::Altered(text.to_owned()), Some(4));
        // A4, the fifth of D minor — the example the issue is written around.
        assert_eq!(voiced(&plain(5), Some("D minor")).expect("voices"), 69.0);
        // C#5, the leading tone a minor key cannot say any other way.
        assert_eq!(voiced(&altered("#7"), Some("D minor")).expect("v"), 73.0);
        // Eb4, the flat second, borrowed into a major key.
        assert_eq!(voiced(&altered("b2"), Some("D major")).expect("v"), 63.0);
        // A string that is only a number is the same as the number.
        assert_eq!(
            voiced(&altered("5"), Some("D minor")).expect("v"),
            voiced(&plain(5), Some("D minor")).expect("v")
        );
    }

    /// An absent octave is a documented default rather than a guess, and it is
    /// the one the field's own doc names.
    #[test]
    fn an_absent_octave_puts_the_tonic_at_four() {
        let written = degree(Degree::Plain(1), None);
        assert_eq!(voiced(&written, Some("C major")).expect("voices"), 60.0);
        assert_eq!(DEFAULT_OCT, 4, "the doc on `oct` says four");
    }

    /// The two refusals that are decisions rather than omissions.
    #[test]
    fn a_degree_needs_a_key_and_needs_to_count_from_one() {
        assert!(matches!(
            voiced(&degree(Degree::Plain(5), Some(4)), None),
            Err(SynthError::DegreeWithoutKey { .. })
        ));
        // The refusal quotes the degree **as the document spelled it**, which
        // is the whole of the fix it hands back — a message naming some other
        // degree, or none, sends the reader to the wrong line.
        for (written, quoted) in [
            (Degree::Plain(0), "0"),
            (Degree::Plain(-1), "-1"),
            (Degree::Altered("0".to_owned()), "0"),
            (Degree::Altered("3b".to_owned()), "3b"),
            (Degree::Altered(String::new()), ""),
            (Degree::Altered("s3".to_owned()), "s3"),
        ] {
            let refusal = voiced(&degree(written.clone(), Some(4)), Some("C major"));
            assert_eq!(
                refusal,
                Err(SynthError::BadDegree {
                    degree: quoted.to_owned()
                }),
                "`{written:?}` should have been refused by name"
            );
        }
    }

    /// Refused rather than clamped, the same way a chord voiced off the
    /// keyboard is: the octave is in the same entry and can be corrected.
    #[test]
    fn a_degree_off_the_keyboard_is_refused() {
        assert_eq!(
            voiced(
                &degree(Degree::Altered("#9".to_owned()), Some(9)),
                Some("C major")
            ),
            Err(SynthError::DegreeOutOfRange {
                degree: "#9".to_owned(),
                oct: 9,
                midi: 135,
            })
        );
    }
}
