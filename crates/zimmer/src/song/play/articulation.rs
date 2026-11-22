//! How a note is played, as opposed to which note it is.

use serde::{Deserialize, Serialize};

/// Multiplies the written velocity; one at 1.0 clamps unaccented, not refused: `vel_scale` may
/// leave headroom.
const ACCENT_VELOCITY: f32 = 1.3;

const ACCENT_BRIGHTNESS: f32 = 0.15;

const STACCATO_GATE: f32 = 0.5;

const GHOST_VELOCITY: f32 = 0.35;

const GHOST_GATE: f32 = 0.4;

const GHOST_BRIGHTNESS: f32 = -0.4;

/// Seconds, not beats: a hand arriving early is no earlier at a slow tempo.
const GHOST_EARLY: f32 = -0.012;

/// Seconds, as `GHOST_EARLY` is: a fast bass slide, heard as one note arriving.
const GLIDE_SECONDS: f32 = 0.06;

/// The slide's cap, as a fraction of the gate, so a short note still ends on its written pitch.
const GLIDE_GATE: f32 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Articulation {
    Accent,
    Staccato,
    Ghost,
    Glide,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Stroke {
    pub(crate) velocity: f32,
    pub(crate) gate: f32,
    /// A fraction of the played velocity, heard only through `vel_octaves` and `vel_index`.
    brightness: f32,
    pub(crate) onset_seconds: f32,
    slides: bool,
}

impl Stroke {
    pub(crate) const PLAIN: Self = Self {
        velocity: 1.0,
        gate: 1.0,
        brightness: 0.0,
        onset_seconds: 0.0,
        slides: false,
    };

    pub(crate) fn of(articulation: Option<Articulation>) -> Self {
        articulation.map_or(Self::PLAIN, Articulation::stroke)
    }

    pub(crate) fn timbre(self, played: f32) -> f32 {
        played * self.brightness
    }

    pub(crate) fn slide_seconds(self, gate: f32) -> Option<f32> {
        self.slides.then(|| GLIDE_SECONDS.min(gate * GLIDE_GATE))
    }
}

impl Articulation {
    pub(crate) fn stroke(self) -> Stroke {
        match self {
            Self::Accent => Stroke {
                velocity: ACCENT_VELOCITY,
                brightness: ACCENT_BRIGHTNESS,
                ..Stroke::PLAIN
            },
            Self::Staccato => Stroke {
                gate: STACCATO_GATE,
                ..Stroke::PLAIN
            },
            Self::Ghost => Stroke {
                velocity: GHOST_VELOCITY,
                gate: GHOST_GATE,
                brightness: GHOST_BRIGHTNESS,
                onset_seconds: GHOST_EARLY,
                ..Stroke::PLAIN
            },
            Self::Glide => Stroke {
                slides: true,
                ..Stroke::PLAIN
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_articulation_is_the_identity() {
        let plain = Stroke::of(None);
        assert_eq!(plain, Stroke::PLAIN);
        assert_eq!(plain.velocity, 1.0);
        assert_eq!(plain.gate, 1.0);
        assert_eq!(plain.timbre(0.8), 0.0);
        assert_eq!(plain.onset_seconds, 0.0);
        assert_eq!(plain.slide_seconds(1.0), None);
    }

    #[test]
    fn a_glide_slides_and_changes_nothing_else() {
        let slid = Stroke::of(Some(Articulation::Glide));
        assert_eq!(slid.slide_seconds(1.0), Some(GLIDE_SECONDS));
        assert_eq!(slid.velocity, 1.0, "a glide is not a louder note");
        assert_eq!(slid.gate, 1.0, "nor a shorter one");
        assert_eq!(slid.timbre(0.6), 0.0, "nor a brighter one");
        assert_eq!(slid.onset_seconds, 0.0, "nor a displaced one");
    }

    #[test]
    fn the_other_marks_do_not_slide() {
        for mark in [
            Articulation::Accent,
            Articulation::Staccato,
            Articulation::Ghost,
        ] {
            assert_eq!(Stroke::of(Some(mark)).slide_seconds(1.0), None, "{mark:?}");
        }
    }

    #[test]
    fn a_slide_onto_a_short_note_is_shortened_with_it() {
        let slid = Stroke::of(Some(Articulation::Glide));
        assert_eq!(slid.slide_seconds(0.04), Some(0.02), "half of a short gate");
        assert_eq!(slid.slide_seconds(10.0), Some(GLIDE_SECONDS), "not scaled");
    }

    #[test]
    fn an_accent_is_harder_and_brighter_and_on_time() {
        let accent = Stroke::of(Some(Articulation::Accent));
        assert!(accent.velocity > 1.0, "an accent is struck harder");
        assert!(accent.timbre(0.6) > 0.0, "and opens the instrument up");
        assert_eq!(accent.gate, 1.0, "an accent is not a shorter note");
        assert_eq!(accent.onset_seconds, 0.0, "nor a displaced one");
    }

    #[test]
    fn staccato_shortens_the_gate_and_nothing_else() {
        let short = Stroke::of(Some(Articulation::Staccato));
        assert!(short.gate < 1.0, "staccato is held short");
        assert_eq!(short.velocity, 1.0, "and at the velocity written");
        assert_eq!(short.timbre(0.6), 0.0, "with the tone written");
        assert_eq!(short.onset_seconds, 0.0, "where it was written");
    }

    #[test]
    fn a_ghost_is_quiet_and_short_and_dull_and_early() {
        let ghost = Stroke::of(Some(Articulation::Ghost));
        assert!(ghost.velocity < 1.0, "a ghost is quiet");
        assert!(ghost.gate < 1.0, "and short");
        assert!(ghost.timbre(0.6) < 0.0, "and dull");
        assert!(ghost.onset_seconds < 0.0, "and early");
    }

    #[test]
    fn a_ghost_is_shorter_than_a_staccato_note() {
        let ghost = Stroke::of(Some(Articulation::Ghost));
        let short = Stroke::of(Some(Articulation::Staccato));
        assert!(ghost.gate < short.gate);
    }

    #[test]
    fn the_brightness_offset_is_a_fraction_of_the_velocity_played() {
        for mark in [Articulation::Accent, Articulation::Ghost] {
            let stroke = Stroke::of(Some(mark));
            let loud = stroke.timbre(1.0);
            assert!((stroke.timbre(0.25) - loud * 0.25).abs() < 1e-6);
            assert!(loud.abs() > 1e-6, "{mark:?} moves the brightness at all");
        }
    }

    #[test]
    fn the_marks_are_spelled_as_the_page_spells_them() {
        for (mark, written) in [
            (Articulation::Accent, "\"accent\""),
            (Articulation::Staccato, "\"staccato\""),
            (Articulation::Ghost, "\"ghost\""),
            (Articulation::Glide, "\"glide\""),
        ] {
            let json = serde_json::to_string(&mark).expect("a mark serialises");
            assert_eq!(json, written);
            let read: Articulation = serde_json::from_str(written).expect("reads back");
            assert_eq!(read, mark);
        }
        assert!(serde_json::from_str::<Articulation>("\"legato\"").is_err());
    }
}
