//! A percussion part as a step string: `"x-x-x-x-x-x-x-xX"`.
//!
//! Sixteen hi-hats used to be sixteen [`Note`]s of five fields each, and that
//! cost is only half of what was wrong with it. The other half is that a list
//! of objects with float `start` values **cannot be read as a rhythm**. A step
//! string can: the hits and the gaps are where they sound, at the size and
//! spacing they sound at, and a reader takes in a whole bar without counting
//! anything. Most of the work on a piece of music is revision, and revision
//! needs the shape to be visible — which is why trackers wrote patterns this
//! way for thirty years.
//!
//! Expansion happens **before** the performance, exactly as a [chord's
//! does](super::chord): every hit is an ordinary note by the time the renderer
//! sees it, so it takes its own ordinal and therefore its own render seed, its
//! own swing displacement and its own humanise nudge. A step string is a way of
//! *writing* notes, not a second kind of thing for the renderer to know about.
//!
//! ## Four characters, and no fifth
//!
//! `x` is a hit, `X` is an accent, `o` is a ghost, `-` is a rest. Anything
//! else is refused, including the bar separators (`|`) and whitespace a
//! tracker screen would have drawn for you: **every character in the string is
//! one step**, the count is load-bearing (see the grid below), and a character
//! that looked like a step but was not would break the one property that makes
//! the notation worth having.
//!
//! ## Two levels and a mark, told apart by shape
//!
//! Digits (`0`–`9`) and a parallel accent string are both more expressive and
//! both cost the thing the string is for: `4-4-4-4-4-4-4-49` makes the eye
//! compare digits to find the accent where `x-x-x-x-x-x-x-xX` shows it as a
//! silhouette, and a second string underneath has to be counted against the
//! first. That argument is against *magnitudes*, not against a third symbol:
//! in `X-o-x-o-` the accents still stand up out of the line and the ghosts sit
//! under it, so the bar is still read as a shape.
//!
//! `x` and `X` are **levels**. An accent is the hardest an instrument is
//! struck, and the format already has a name for that — velocity 1, where a
//! note's `vel` tops out. So `X` is 1 and `x` is the entry's
//! [`vel`](Steps::vel), which puts the **ratio between them in the document**,
//! written with the one number the writer already knows: `vel: 0.4` is a quiet
//! hat with a hard accent, `vel: 0.85` is a nearly even one.
//!
//! `o` is not a third level: it is the [`ghost`](Articulation::Ghost) **mark**
//! on that one hit — quiet, short, dull and a hair early, four things a hand
//! does together. A level alone would be a quiet hit, which is exactly what a
//! ghost is not, and it would need a velocity nobody could name. As a mark it
//! derives from `vel` the way every mark derives from the velocity it is
//! written at, so nothing new is written. The mark is the hit's own, so it
//! stands in for the entry's [`articulation`](Steps::articulation) on that
//! step: the nearer instruction wins, as a ghost inside a passage marked
//! staccato is still played as a ghost.
//!
//! Two degenerate readings are [refused rather than
//! played](SynthError::AccentWithoutHeadroom). A string carrying both cases
//! while `vel` is 1 draws a distinction the entry gave it no room to make, and
//! the audio would silently not have the accents the page shows. And an entry
//! marked with the word the string already spells — `accent` beside an `X`,
//! `ghost` beside an `o` — says it twice in two vocabularies.
//!
//! ## No holds: `dur` is a field
//!
//! `x---` is a hit and three rests, never one note four steps long. The held
//! form is nicer to write and it is ambiguous in exactly the place percussion
//! lives — a drum part is mostly rests, so the reading that makes `-` a
//! continuation makes every gap in the notation mean two things. A distinct
//! hold character (`x===`) would resolve that, and it is still not here: a step
//! string is percussion notation, and a percussion hit's length is its patch's
//! decay rather than its gate. One [`dur`](Steps::dur) for every hit, defaulting
//! to the step, is what a step sequencer means by a step.
//!
//! If a real part wants one long note under a groove, that note is one
//! hand-written [`Note`] beside the string. A hold character stays purely
//! additive if a piece ever asks for one; removing it later would not be.
//!
//! ## One pitch, and no melody
//!
//! [`note`](Steps::note) is per entry, because a percussion track plays one
//! sound throughout — and it stays per entry. A step string is not the place to
//! express a melody: pitch per step would need a character per pitch, which is
//! the tracker's two-column row, and at that point the notation is no longer
//! legible as a shape and the entry is no longer a percussion part. Melody is
//! written as notes.
//!
//! ## The grid, stated twice on purpose
//!
//! [`div`](Steps::div) is the step length in beats, and the string must cover
//! the pattern **exactly** — from its `start` to the end of the slot, rests
//! included. So `div` is redundant: it could be derived from the length and the
//! slot. The redundancy *is* the check. A string one character short is the
//! error nobody sees — fifteen sixteenths read as a bar until the ear finds it
//! — and comparing the count the writer typed against the count the grid needs
//! is the only thing that catches it. Deriving `div` instead would turn that
//! typo into a silently different tempo of hi-hat.
//!
//! Covering the whole slot is the same rule seen from the other side: a figure
//! that occupies two beats of an eight-beat pattern is written with its rests,
//! so the string always shows the bar it sits in rather than a fragment whose
//! position has to be read off a `start` field.
//!
//! ## What is not here: a Euclidean generator
//!
//! `{ "hits": 5, "steps": 16 }` is elegant, it is famous, and it is **refused
//! by the point of this module**. The string exists so a rhythm can be seen; a
//! generator produces a rhythm nobody can see without running the algorithm in
//! their head, and it saves eleven characters over writing out the thing it
//! would have produced. It is the judgement `docs/recipes.md` already applies to
//! inversion and retrograde — a real operation, and the one nobody reaches for
//! by hand. If a real piece wants one, that is a request with the piece
//! attached.

use serde::{Deserialize, Serialize};

use super::{Articulation, Note, Pitch, one};
use crate::error::SynthError;

/// A step that sounds at the entry's own velocity.
const HIT: char = 'x';

/// A step that sounds at full velocity — see the module doc on why the accent
/// is the fixed end of the pair and the plain hit is the written one.
const ACCENT: char = 'X';

/// A step played as a [ghost](Articulation::Ghost) of a plain hit — see the
/// module doc on why it is a mark and not a third level.
const GHOST: char = 'o';

/// A step that does not sound.
const REST: char = '-';

/// What an [`ACCENT`] plays at: full velocity, the top of the range a note's
/// `vel` already lives in.
const ACCENT_VEL: f32 = 1.0;

/// Where a hit sits when the entry does not say: middle C, `C4`, the reference
/// the rest of the format measures pitch from.
///
/// A noise-based drum ignores it entirely, which is the common case. A *tuned*
/// percussion patch — a sine kick, a woodblock — should say what it plays,
/// because no default is right for both.
const DEFAULT_PITCH: f32 = 60.0;

/// How far a beat grid may miss the mark and still be called exact, in beats.
///
/// Shared with [an arpeggio's](super::chord::arp) step count, which divides a
/// chord's length the same way and inherits the same representation problem.
///
/// Not zero, because a triplet grid is not representable: twelve steps of a
/// third of a beat land a hair off four beats in `f32` and are obviously
/// correct. A thousandth of a beat is under half a millisecond at any tempo
/// anybody writes, and a step count off by one misses by a whole `div`.
pub(super) const GRID_EPSILON: f32 = 1e-3;

/// True when a step string starts at the top of its pattern, which is where
/// almost all of them start — kept out of the saved document so the common
/// entry is three fields.
fn at_the_top(start: &f32) -> bool {
    *start == 0.0
}

/// A run of evenly spaced hits, written as one string over a stated grid.
///
/// Every field but `steps` and `div` means what it means on a [`Note`], because
/// every hit becomes one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Steps {
    /// The [`Track::name`](super::Track::name) that plays it.
    pub track: String,
    /// The pattern itself: `x` a hit, `X` an accent, `o` a ghost, `-` a rest,
    /// and nothing else. One character is one step of [`div`](Self::div) beats.
    pub steps: String,
    /// How long one step is, in beats: `0.5` is an eighth, `0.25` a sixteenth.
    ///
    /// Stated even though the string's length and the pattern's slot would
    /// determine it — that redundancy is what catches a string one character
    /// short, which is the typo this notation would otherwise hide.
    pub div: f32,
    /// Onset of the **first step** in beats from the start of the pattern.
    /// Absent means 0, and the string runs from there to the end of the slot.
    #[serde(default, skip_serializing_if = "at_the_top")]
    pub start: f32,
    /// Gate length in beats for every hit. Absent means one step, which is
    /// what a step sequencer means by a step; a percussive patch's decay
    /// usually ends the sound well before the gate does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dur: Option<f32>,
    /// What pitch every hit plays. Absent means middle C — see the module doc
    /// on why pitch is per entry and never per step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<Pitch>,
    /// Velocity of a plain `x`, in `0..=1`. An `X` plays at 1 regardless, so
    /// this is the *distance* between an ordinary hit and an accented one; an
    /// `o` is this hit with the ghost mark on it.
    #[serde(default = "one")]
    pub vel: f32,
    /// How **every** hit is played — see [`Note::articulation`]. A run is one
    /// gesture repeated, so this says how the hand plays the whole of it: a
    /// staccato hat part, a ghosted snare run.
    ///
    /// An `o` step is the exception: it is played as a ghost whatever this
    /// says. The two combinations refused are
    /// [`accent` beside an `X`](SynthError::TwiceAccented) and
    /// [`ghost` beside an `o`](SynthError::TwiceGhosted), because the string
    /// already has a way to say each and the entry cannot say it again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub articulation: Option<Articulation>,
}

impl Steps {
    /// Gate length of every hit: what the entry says, or one step.
    pub(crate) fn gate(&self) -> f32 {
        self.dur.unwrap_or(self.div)
    }

    /// Appends the notes this string sounds to `out`, or says why it cannot.
    ///
    /// `beats` is the slot of the pattern holding it, which the grid is checked
    /// against — see the module doc. Nothing is appended when the entry is
    /// refused: a half-expanded string would be worse than a rejected one.
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
        // Checked after the characters, so a string with both faults reports
        // the one that is a typo before the one that is a misunderstanding.
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

    /// That the string covers the pattern exactly, from `start` to the end of
    /// the slot.
    ///
    /// Two different faults, told apart because they need different fixes: a
    /// `div` no whole number of steps fits into is a grid that does not belong
    /// to this pattern, and a count that misses the one the grid needs is a
    /// string to add a character to.
    fn check_grid(&self, beats: f32, written: usize) -> Result<(), SynthError> {
        let where_it_is = || (self.track.clone(), self.start, self.div, beats);
        let needed = ((beats - self.start) / self.div).round();
        // Non-finite and non-positive values fall out here rather than being
        // tested for: a `div` of zero or a NaN slot cannot produce a count that
        // lands back on `beats`.
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

    /// A step entry over an eight-beat pattern, as most of them are written.
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

    /// What a string expands to: the onset and velocity of each hit.
    fn hits(steps: &Steps, beats: f32) -> Result<Vec<(f32, f32)>, SynthError> {
        let mut out = Vec::new();
        steps.voice_into(beats, &mut out)?;
        Ok(out.iter().map(|note| (note.start, note.vel)).collect())
    }

    /// The onsets the notation claims, stated as numbers.
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

    /// The gate is one step unless the entry says otherwise, and the pitch is
    /// middle C unless it does.
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

    /// A string starting late still runs to the end of the slot, so the rests
    /// before it are what put it where it is.
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

    /// The whole reason the character set is closed.
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

    /// A count that is not the count the grid needs, which is the typo this
    /// notation would otherwise hide.
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

    /// A grid no whole number of steps fits into is its own fault, because it
    /// is `div` that has to change rather than the string.
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
        // A triplet grid is not exactly representable and is obviously right.
        let triplets = Steps {
            div: 1.0 / 3.0,
            ..steps("xxxxxxxxxxxx")
        };
        assert!(
            hits(&triplets, 4.0).is_ok(),
            "twelve triplets fill four beats"
        );
    }

    /// An accent that cannot be louder than the hits beside it is a
    /// distinction the audio would silently not make.
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
        // Accents alone lose nothing: there is no plain hit to be softer.
        let all_accents = Steps {
            vel: 1.0,
            ..steps("X-X-X-X-X-X-X-X-")
        };
        assert!(hits(&all_accents, 8.0).is_ok());
    }

    /// An `o` is a plain hit's velocity with the ghost mark on it, and the mark
    /// is the hit's own: the entry's articulation reaches the other hits only.
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

    /// Nothing reaches the caller from an entry that is refused — a
    /// half-expanded string is worse than a rejected one.
    #[test]
    fn a_refused_string_appends_nothing() {
        let mut out = Vec::new();
        assert!(steps("x-x-x?x-x-x-x-x-").voice_into(8.0, &mut out).is_err());
        assert!(out.is_empty(), "hits before the bad character leaked out");
    }
}
