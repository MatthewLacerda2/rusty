//! How a note is played, as opposed to which note it is.
//!
//! A [`Note`](super::Note) says pitch, onset, length and force. That is the
//! whole of *what* is played and none of *how*, and the gap is most audible in
//! the place a score most often sounds sequenced. A bassline is the clear case:
//! what makes one sound played rather than programmed is almost entirely
//! articulation — a slide up into the root of the bar, a ghost note between the
//! real ones, a couple of accents carrying the groove, a staccato stop where
//! the phrase breathes.
//!
//! **This is not the same thing as [`feel`](super::feel), and the difference is
//! the reason both exist.** Swing and humanise scatter a performance the player
//! did not intend — where the beat is, and how well he hit it. An articulation
//! is what he *did* intend, written on the page. They compose rather than
//! overlap, and the order they compose in is stated in
//! [`render`](super::render).
//!
//! ## Named, never numbers
//!
//! Three of these are a combination of things the engine already does:
//! velocity, gate length, the velocity-to-brightness routings a patch may carry
//! (`vel_octaves`, `vel_index`), and where the note sits against the beat. A
//! document could write those numbers itself — and then nothing would record
//! that they were an *accent*, every accent in the piece would be a slightly
//! different one, and changing what an accent means would be an edit to every
//! note carrying one. A name says what was meant, and the meaning lives in one
//! place: the table of constants below.
//!
//! **A glide is the fourth and it is different in kind.** It is not a
//! rearrangement of anything a note carries, because the pitch it starts on is
//! not a property of that note at all: it belongs to the note before it on the
//! same track. This module still owns what the *mark* means — how long the
//! slide takes — while [`super::glide`] owns the harder half, which is what
//! "the note before it" is allowed to mean.
//!
//! ## The set is closed, and it is four
//!
//! `accent`, `staccato`, `ghost`, `glide`. The test for a fifth is the one
//! `docs/recipes.md` already applies to inversion and retrograde: whether a
//! person reaches for it by hand.
//!
//! ## One articulation, not a set of them
//!
//! An entry carries at most one, so `["accent", "staccato"]` is not writable.
//! Each name here is a whole gesture rather than a modifier — a ghost is
//! already short, an accent is already full length — so a set would mostly
//! express contradictions (an accented ghost) and would need an order of
//! application between its members to mean anything. The one real combination
//! it costs is *short and hard*, and that one is a written `dur` beside an
//! `accent`, which is what the field is for.

use serde::{Deserialize, Serialize};

/// How much harder than written an accent is struck.
///
/// A multiplier rather than a target, so an accent keeps the dynamic the
/// document wrote instead of flattening every accented note onto one level —
/// the same shape [`vel_scale`](super::Play::vel_scale) and
/// [`Humanize::velocity`](super::Humanize::velocity) already have, which is
/// what lets the three multiply in any combination without arguing.
///
/// A note already written at full velocity therefore comes out unaccented,
/// because full is full and the note renderer clamps there. That is the rule
/// `humanize` already lives by — a piece that wants dynamics writes its notes
/// below `1.0` — and it is deliberately **documented rather than refused**: a
/// note at `1.0` in a section carrying `"vel_scale": 0.6` has all the headroom
/// it needs, and a refusal read off the written value alone would refuse it.
const ACCENT_VELOCITY: f32 = 1.3;

/// How much brighter an accent is, as a fraction of the velocity it ends up
/// played at.
///
/// An accent is not merely a louder note: leaning on a string or hitting a key
/// squarely opens the instrument up as well. It reaches a patch through the two
/// routings that already read velocity as effort, so a patch naming neither
/// hears only the level — the same silence
/// [`Humanize::timbre`](super::Humanize::timbre) accepts, and for the same
/// reason: what "brighter" means belongs to the instrument.
const ACCENT_BRIGHTNESS: f32 = 0.15;

/// What fraction of its written `dur` a staccato note is actually held for.
///
/// Half, which is what the mark has meant on paper for two centuries. **The
/// written `dur` is untouched**, which is the whole point of spelling the
/// articulation instead of halving the number: the rhythm on the page stays the
/// rhythm of the music, and a phrase of quarter notes played short still reads
/// as quarter notes.
///
/// Fixed rather than a parameter. A fraction per note is the raw number this
/// module exists to replace, and a document that genuinely wants one specific
/// shorter length is writing a `dur`, which it can already do.
const STACCATO_GATE: f32 = 0.5;

/// How quiet a ghost is, as a fraction of the velocity written.
const GHOST_VELOCITY: f32 = 0.35;

/// What fraction of its written `dur` a ghost is held for.
///
/// Shorter than [`STACCATO_GATE`], and the two are separate numbers rather than
/// one shared constant because they are separate claims. A staccato note is a
/// note being played short; a ghost is barely a note at all.
const GHOST_GATE: f32 = 0.4;

/// How much duller a ghost is, as a fraction of the velocity it is played at.
///
/// Negative, and it is the field that makes a ghost *dead* rather than merely
/// quiet. Turning a note down alone leaves it the same bright thing further
/// away; a ghost is a muted string, so it loses its top as well.
const GHOST_BRIGHTNESS: f32 = -0.4;

/// Where a ghost sits against the beat, in seconds — negative, because it sits
/// ahead of it.
///
/// Seconds rather than beats, for the reason
/// [`Humanize::timing`](super::Humanize::timing) is in seconds: this is a hand
/// arriving early, not a subdivision, and it does not get three times wider
/// when the piece is played at 40 bpm. Twelve milliseconds is under a
/// thirty-second note at any tempo anybody writes and is plainly audible as
/// *ahead* rather than as a different rhythm.
const GHOST_EARLY: f32 = -0.012;

/// How long a glide takes to arrive on the note it is written over, in
/// seconds.
///
/// Seconds rather than beats, for the reason [`GHOST_EARLY`] is in seconds: a
/// slide is a hand crossing a distance, and it does not take three times as
/// long because the piece is played at 40 bpm. Sixty milliseconds is the fast
/// slide a bass player makes — long enough to hear as one note arriving from
/// somewhere rather than as two notes, short enough that the note is on its
/// own pitch for the part of it anybody is listening to.
///
/// Fixed rather than a parameter, for the reason [`STACCATO_GATE`] is fixed: a
/// time per note is the raw number this module exists to replace.
const GLIDE_SECONDS: f32 = 0.06;

/// The most of a note's gate a glide may spend arriving.
///
/// A cap, not a second opinion about the time. A sixteenth at 140 bpm is a
/// hundred milliseconds, and a slide still moving when the gate shuts is a
/// note that never played what the page said it was — a run of them is a part
/// with no pitches in it. Half leaves the back half of every note on the
/// written pitch, however short the note.
const GLIDE_GATE: f32 = 0.5;

/// How a note is played: the mark a score would write over it.
///
/// One of a closed set of four; this module's own doc says what the test for
/// a fifth is, and why they are named rather than spelled as numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Articulation {
    /// Struck harder than written, and brighter with it — the note that
    /// carries the groove.
    Accent,
    /// Held for half its written length, which stays written — the stop where
    /// a phrase breathes.
    Staccato,
    /// Quiet, short, dull and a hair early — the note between the notes.
    Ghost,
    /// Slid onto from the pitch of the note before it on the track — the
    /// gesture a bassline is half made of. Which note that is, and what
    /// "before" means, is the song renderer's to decide; the first note of a
    /// track has none and is played plain.
    Glide,
}

/// What an articulation does to one note: four numbers, and whether it slides.
///
/// One lookup per note rather than five, and one place where the whole meaning
/// of a mark is visible at once. [`PLAIN`](Self::PLAIN) is the absent
/// articulation, so the renderer has a single path rather than a branch per
/// note — the same shape an absent [`Humanize`](super::Humanize) already takes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Stroke {
    /// Multiplies the velocity the note is struck at.
    pub(crate) velocity: f32,
    /// Multiplies the gate, leaving the written `dur` alone.
    pub(crate) gate: f32,
    /// Moves the brightness routings away from the level, as a fraction of the
    /// velocity played — positive is brighter. Read through
    /// [`Stroke::timbre`].
    brightness: f32,
    /// Moves the onset, in seconds; negative is early.
    pub(crate) onset_seconds: f32,
    /// Whether the note is slid onto rather than struck on its own pitch. A
    /// flag rather than a number because *how far* is not the mark's to say —
    /// it is wherever the previous note left the hand. Read through
    /// [`Stroke::slide_seconds`].
    slides: bool,
}

impl Stroke {
    /// A note played exactly as written, which is what an absent articulation
    /// means.
    pub(crate) const PLAIN: Self = Self {
        velocity: 1.0,
        gate: 1.0,
        brightness: 0.0,
        onset_seconds: 0.0,
        slides: false,
    };

    /// What an entry's articulation does, or [`PLAIN`](Self::PLAIN) if it
    /// carries none.
    pub(crate) fn of(articulation: Option<Articulation>) -> Self {
        articulation.map_or(Self::PLAIN, Articulation::stroke)
    }

    /// How far to move the velocity the *brightness* routings see, given the
    /// velocity `played` this note is actually struck at.
    ///
    /// A fraction of the played velocity rather than of full scale, which is
    /// the form [`Humanize::timbre`](super::Humanize::timbre) already uses and
    /// is why the two can simply be added: both are offsets in the same units,
    /// against the same number.
    pub(crate) fn timbre(self, played: f32) -> f32 {
        played * self.brightness
    }

    /// How long this note spends sliding onto its pitch, given the `gate` in
    /// seconds it is held for — `None` for a mark that does not slide, which
    /// is every mark but one.
    ///
    /// The gate is an argument because [`GLIDE_GATE`] caps the slide against
    /// it, and how long the note is held is a fact about the tempo and the
    /// written `dur` rather than about the mark — the same sixteenth is 107 ms
    /// at 140 bpm and 375 ms at 40.
    pub(crate) fn slide_seconds(self, gate: f32) -> Option<f32> {
        self.slides.then(|| GLIDE_SECONDS.min(gate * GLIDE_GATE))
    }
}

impl Articulation {
    /// What this mark stands for.
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

    /// An absent mark changes nothing at all — the note the renderer rendered
    /// before this module existed.
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

    /// A glide is the pitch and only the pitch: the note is struck exactly as
    /// written, and the one thing that moves is where it comes from.
    #[test]
    fn a_glide_slides_and_changes_nothing_else() {
        let slid = Stroke::of(Some(Articulation::Glide));
        assert_eq!(slid.slide_seconds(1.0), Some(GLIDE_SECONDS));
        assert_eq!(slid.velocity, 1.0, "a glide is not a louder note");
        assert_eq!(slid.gate, 1.0, "nor a shorter one");
        assert_eq!(slid.timbre(0.6), 0.0, "nor a brighter one");
        assert_eq!(slid.onset_seconds, 0.0, "nor a displaced one");
    }

    /// Nothing else slides. A mark that quietly did would be a portamento
    /// nobody wrote, on the note after every ghost.
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

    /// A slide never spends more than half the note arriving, so a short one
    /// still plays the pitch it was written at — and a long one is the plain
    /// slide, not a fraction of itself.
    #[test]
    fn a_slide_onto_a_short_note_is_shortened_with_it() {
        let slid = Stroke::of(Some(Articulation::Glide));
        assert_eq!(slid.slide_seconds(0.04), Some(0.02), "half of a short gate");
        assert_eq!(slid.slide_seconds(10.0), Some(GLIDE_SECONDS), "not scaled");
    }

    /// An accent is louder **and** brighter, and it is nothing else: it does
    /// not shorten the note and it does not move it.
    #[test]
    fn an_accent_is_harder_and_brighter_and_on_time() {
        let accent = Stroke::of(Some(Articulation::Accent));
        assert!(accent.velocity > 1.0, "an accent is struck harder");
        assert!(accent.timbre(0.6) > 0.0, "and opens the instrument up");
        assert_eq!(accent.gate, 1.0, "an accent is not a shorter note");
        assert_eq!(accent.onset_seconds, 0.0, "nor a displaced one");
    }

    /// Staccato is the gate and only the gate: the same note, held shorter.
    #[test]
    fn staccato_shortens_the_gate_and_nothing_else() {
        let short = Stroke::of(Some(Articulation::Staccato));
        assert!(short.gate < 1.0, "staccato is held short");
        assert_eq!(short.velocity, 1.0, "and at the velocity written");
        assert_eq!(short.timbre(0.6), 0.0, "with the tone written");
        assert_eq!(short.onset_seconds, 0.0, "where it was written");
    }

    /// The whole reason a ghost is not just a quiet note: every one of the four
    /// numbers moves, and each in the direction the name claims.
    #[test]
    fn a_ghost_is_quiet_and_short_and_dull_and_early() {
        let ghost = Stroke::of(Some(Articulation::Ghost));
        assert!(ghost.velocity < 1.0, "a ghost is quiet");
        assert!(ghost.gate < 1.0, "and short");
        assert!(ghost.timbre(0.6) < 0.0, "and dull");
        assert!(ghost.onset_seconds < 0.0, "and early");
    }

    /// A ghost is shorter than a staccato note, which is the claim
    /// [`GHOST_GATE`]'s own doc makes and the one a shared constant would
    /// silently drop.
    #[test]
    fn a_ghost_is_shorter_than_a_staccato_note() {
        let ghost = Stroke::of(Some(Articulation::Ghost));
        let short = Stroke::of(Some(Articulation::Staccato));
        assert!(ghost.gate < short.gate);
    }

    /// The brightness offset is a fraction of the velocity the note is played
    /// at, so a soft strike strays less in absolute terms than a hard one —
    /// the form that lets it be added to the humanise offset.
    #[test]
    fn the_brightness_offset_is_a_fraction_of_the_velocity_played() {
        for mark in [Articulation::Accent, Articulation::Ghost] {
            let stroke = Stroke::of(Some(mark));
            let loud = stroke.timbre(1.0);
            assert!((stroke.timbre(0.25) - loud * 0.25).abs() < 1e-6);
            assert!(loud.abs() > 1e-6, "{mark:?} moves the brightness at all");
        }
    }

    /// The names a document writes, which are the names the page documents.
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
