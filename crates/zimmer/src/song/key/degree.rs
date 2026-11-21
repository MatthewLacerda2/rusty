//! A note written as a degree of the song's key rather than as a pitch.

use serde::{Deserialize, Serialize};

use super::Key;
use crate::error::SynthError;
use crate::note::MIDI_RANGE;
use crate::song::{one, Articulation, Note, Pitch};

const DEFAULT_OCT: i32 = 4;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DegreeNote {
    pub track: String,
    pub degree: Degree,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oct: Option<i32>,
    pub start: f32,
    pub dur: f32,
    #[serde(default = "one")]
    pub vel: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub articulation: Option<Articulation>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Degree {
    Plain(i32),
    Altered(String),
}

impl Degree {
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
        if degree < 1 {
            return Err(bad());
        }
        Ok((degree, alteration))
    }

    fn written(&self) -> String {
        match self {
            Self::Plain(degree) => degree.to_string(),
            Self::Altered(text) => text.clone(),
        }
    }
}

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

    fn voiced(note: &DegreeNote, key: Option<&str>) -> Result<f32, SynthError> {
        let key = key.map(|text| Key::parse(text).expect("a key"));
        let mut out = Vec::new();
        note.voice_into(key.as_ref(), &mut out)?;
        out[0].note.to_midi()
    }

    #[test]
    fn a_degree_is_the_scale_note_it_names() {
        let plain = |number| degree(Degree::Plain(number), Some(4));
        let altered = |text: &str| degree(Degree::Altered(text.to_owned()), Some(4));
        assert_eq!(voiced(&plain(5), Some("D minor")).expect("voices"), 69.0);
        assert_eq!(voiced(&altered("#7"), Some("D minor")).expect("v"), 73.0);
        // Eb4, the flat second, borrowed into a major key.
        assert_eq!(voiced(&altered("b2"), Some("D major")).expect("v"), 63.0);
        assert_eq!(
            voiced(&altered("5"), Some("D minor")).expect("v"),
            voiced(&plain(5), Some("D minor")).expect("v")
        );
    }

    #[test]
    fn an_absent_octave_puts_the_tonic_at_four() {
        let written = degree(Degree::Plain(1), None);
        assert_eq!(voiced(&written, Some("C major")).expect("voices"), 60.0);
        assert_eq!(DEFAULT_OCT, 4, "the doc on `oct` says four");
    }

    #[test]
    fn a_degree_needs_a_key_and_needs_to_count_from_one() {
        assert!(matches!(
            voiced(&degree(Degree::Plain(5), Some(4)), None),
            Err(SynthError::DegreeWithoutKey { .. })
        ));
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
