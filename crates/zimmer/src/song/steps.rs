//! A percussion part as a step string: `"x-x-x-x-x-x-x-xX"`.

use serde::{Deserialize, Serialize};

use super::{Articulation, Note, Pitch, one};
use crate::error::SynthError;

const HIT: char = 'x';

const ACCENT: char = 'X';

const GHOST: char = 'o';

const REST: char = '-';

const ACCENT_VEL: f32 = 1.0;

const DEFAULT_PITCH: f32 = 60.0;

pub(super) const GRID_EPSILON: f32 = 1e-3;

fn at_the_top(start: &f32) -> bool {
    *start == 0.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Steps {
    pub track: String,
    pub steps: String,
    pub div: f32,
    #[serde(default, skip_serializing_if = "at_the_top")]
    pub start: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dur: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<Pitch>,
    #[serde(default = "one")]
    pub vel: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub articulation: Option<Articulation>,
}

impl Steps {
    pub(crate) fn gate(&self) -> f32 {
        self.dur.unwrap_or(self.div)
    }

    pub(crate) fn voice_into(&self, beats: f32, out: &mut Vec<Note>) -> Result<(), SynthError> {
        let written = self.steps.chars().count();
        self.check_grid(beats, written)?;
        let pitch = self.note.clone().unwrap_or(Pitch::Midi(DEFAULT_PITCH));
        let mut hits = Vec::new();
        let (mut plain, mut accented, mut ghosted) = (false, false, false);
        for (step, character) in self.steps.chars().enumerate() {
            let (vel, articulation) = match character {
                REST => continue,
                HIT => {
                    plain = true;
                    (self.vel, self.articulation)
                }
                ACCENT => {
                    accented = true;
                    (ACCENT_VEL, self.articulation)
                }
                GHOST => {
                    ghosted = true;
                    (self.vel, Some(Articulation::Ghost))
                }
                _ => {
                    return Err(SynthError::BadStep {
                        track: self.track.clone(),
                        character,
                        step,
                    });
                }
            };
            hits.push(Note {
                track: self.track.clone(),
                note: pitch.clone(),
                start: self.start + step as f32 * self.div,
                dur: self.gate(),
                vel,
                articulation,
            });
        }
        if plain && accented && self.vel >= ACCENT_VEL {
            return Err(SynthError::AccentWithoutHeadroom {
                track: self.track.clone(),
                vel: self.vel,
            });
        }
        if accented && self.articulation == Some(Articulation::Accent) {
            return Err(SynthError::TwiceAccented {
                track: self.track.clone(),
            });
        }
        if ghosted && self.articulation == Some(Articulation::Ghost) {
            return Err(SynthError::TwiceGhosted {
                track: self.track.clone(),
            });
        }
        out.extend(hits);
        Ok(())
    }

    fn check_grid(&self, beats: f32, written: usize) -> Result<(), SynthError> {
        let where_it_is = || (self.track.clone(), self.start, self.div, beats);
        let needed = ((beats - self.start) / self.div).round();
        // Non-finite and non-positive values fall out here rather than being tested for: a `div` of
        // zero or a NaN slot cannot produce a count that lands back on `beats`.
        if !(needed >= 1.0 && (self.start + needed * self.div - beats).abs() <= GRID_EPSILON) {
            let (track, start, div, beats) = where_it_is();
            return Err(SynthError::BadStepDiv {
                track,
                start,
                div,
                beats,
            });
        }
        let needed = needed as usize;
        if written != needed {
            let (track, start, div, beats) = where_it_is();
            return Err(SynthError::StepsDoNotFit {
                track,
                start,
                div,
                beats,
                written,
                needed,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn steps(pattern: &str) -> Steps {
        Steps {
            track: "hat".to_owned(),
            steps: pattern.to_owned(),
            div: 0.5,
            start: 0.0,
            dur: None,
            note: None,
            vel: 0.4,
            articulation: None,
        }
    }

    fn hits(steps: &Steps, beats: f32) -> Result<Vec<(f32, f32)>, SynthError> {
        let mut out = Vec::new();
        steps.voice_into(beats, &mut out)?;
        Ok(out.iter().map(|note| (note.start, note.vel)).collect())
    }

    #[test]
    fn a_string_sounds_where_its_characters_are() {
        assert_eq!(
            hits(&steps("x-x-x-x-x-x-x-xX"), 8.0).expect("expands"),
            vec![
                (0.0, 0.4),
                (1.0, 0.4),
                (2.0, 0.4),
                (3.0, 0.4),
                (4.0, 0.4),
                (5.0, 0.4),
                (6.0, 0.4),
                (7.0, 0.4),
                (7.5, 1.0)
            ]
        );
    }

    #[test]
    fn the_absent_fields_are_the_documented_defaults() {
        let mut out = Vec::new();
        steps("x---------------")
            .voice_into(8.0, &mut out)
            .expect("expands");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].dur, 0.5, "one step");
        assert_eq!(out[0].note, Pitch::Midi(DEFAULT_PITCH));
        assert_eq!(DEFAULT_PITCH, 60.0, "the doc on `note` says middle C");
    }

    #[test]
    fn a_late_string_still_reaches_the_end_of_the_slot() {
        let late = Steps {
            start: 6.0,
            ..steps("x-xX")
        };
        assert_eq!(
            hits(&late, 8.0).expect("expands"),
            vec![(6.0, 0.4), (7.0, 0.4), (7.5, 1.0)]
        );
    }

    #[test]
    fn a_character_that_is_not_a_step_is_refused() {
        for pattern in [
            "x-x-x-x-|x-x-x-x",
            "x.x-x-x-x-x-x-x-",
            "x x-x-x-x-x-x-x-",
            "x-o-x-O-x-o-x-o-",
        ] {
            assert!(
                matches!(hits(&steps(pattern), 8.0), Err(SynthError::BadStep { .. })),
                "`{pattern}` should not have expanded"
            );
        }
    }

    #[test]
    fn a_string_that_does_not_fill_the_pattern_is_refused() {
        for (pattern, written) in [("x-x-x-x-x-x-x-x", 15), ("x-x-x-x-x-x-x-x-x", 17), ("", 0)] {
            assert!(
                matches!(
                    hits(&steps(pattern), 8.0),
                    Err(SynthError::StepsDoNotFit { written: got, needed: 16, .. }) if got == written
                ),
                "`{pattern}` should not have fitted"
            );
        }
    }

    #[test]
    fn a_grid_that_does_not_divide_the_slot_is_refused() {
        for div in [0.3, 0.0, -0.5, f32::NAN, f32::INFINITY] {
            let odd = Steps {
                div,
                ..steps("x-x-")
            };
            assert!(
                matches!(hits(&odd, 8.0), Err(SynthError::BadStepDiv { .. })),
                "a div of {div} should not have divided eight beats"
            );
        }
        let triplets = Steps {
            div: 1.0 / 3.0,
            ..steps("xxxxxxxxxxxx")
        };
        assert!(
            hits(&triplets, 4.0).is_ok(),
            "twelve triplets fill four beats"
        );
    }

    #[test]
    fn an_accent_with_no_room_above_the_plain_hits_is_refused() {
        let flat = Steps {
            vel: 1.0,
            ..steps("x-x-x-x-x-x-x-xX")
        };
        assert!(matches!(
            hits(&flat, 8.0),
            Err(SynthError::AccentWithoutHeadroom { .. })
        ));
        let all_accents = Steps {
            vel: 1.0,
            ..steps("X-X-X-X-X-X-X-X-")
        };
        assert!(hits(&all_accents, 8.0).is_ok());
    }

    #[test]
    fn a_ghost_step_is_a_plain_hit_marked_ghost() {
        let mut out = Vec::new();
        let marked = Steps {
            articulation: Some(Articulation::Staccato),
            ..steps("X-o-x-o-X-o-x-o-")
        };
        marked.voice_into(8.0, &mut out).expect("expands");
        let ghost = Some(Articulation::Ghost);
        let staccato = Some(Articulation::Staccato);
        let played: Vec<_> = out.iter().map(|n| (n.vel, n.articulation)).collect();
        assert_eq!(
            played[..4],
            [(1.0, staccato), (0.4, ghost), (0.4, staccato), (0.4, ghost)]
        );
    }

    #[test]
    fn a_refused_string_appends_nothing() {
        let mut out = Vec::new();
        assert!(steps("x-x-x?x-x-x-x-x-").voice_into(8.0, &mut out).is_err());
        assert!(out.is_empty(), "hits before the bad character leaked out");
    }
}
