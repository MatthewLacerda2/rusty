//! An entry in the running order: a pattern — or several at once — and
//! optionally how to play each.
//!
//! A [`Song`](super::Song) has two ways to relate one section to another, and
//! before this module there were only the bad ones: **play the identical
//! pattern again**, or **write a second pattern out note by note**. The first
//! repeats without developing; the second puts the relationship in the audio
//! and *nowhere in the document*, so nothing records that the last chorus is
//! the first one up a third and an edit to one silently desynchronises the
//! pair.
//!
//! The economics are the sharper argument. The unit of work here is an agent
//! writing JSON, where every note is a five-field object and a dense section is
//! a few hundred lines. If repetition costs one word and variation costs a full
//! pattern rewrite, an agent writes four thin patterns and repeats them — not
//! for lack of imagination but because the format prices variation out. A
//! transform costs less than either, so the format stops punishing the thing we
//! want more of.
//!
//! **This is vocabulary, not composition.** `transpose: 3` is a property type;
//! a module that picked a ii–V–I because it knows that is a good progression
//! would be a property *value*, and the same category error as "make text red".

use serde::{Deserialize, Serialize};

use super::Key;
use crate::note::MIDI_RANGE;

/// One entry in the running order: one slot of the piece, and what plays in
/// it.
///
/// Most entries are one pattern, played as written or transformed — a
/// [`Layer`]. An entry can also stack several, which is how a solo is written
/// once and played over a groove that is also written once: before that form
/// existed, every section that put a new part over the rhythm section had to
/// carry a full copy of the rhythm section's notes (#506).
///
/// Untagged when written, so every song from before either form parses and
/// re-serialises unchanged, short form still short. It is read by hand, by the
/// one key that cannot be misspelled into the other form — `layers` — so a
/// misspelled key in a [`Play`] or a [`Layers`] is refused by name; the
/// `song::forms` module has why.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ArrangementEntry {
    /// One pattern, in a slot exactly as long as it is.
    Single(Layer),
    /// Several patterns at the same time.
    Layered(Layers),
}

/// Several patterns sounding in one slot: `{ "layers": ["groove", "solo"] }`.
///
/// **The slot is as long as its longest layer**, so the next entry starts when
/// everything in this one has had its say, and a shorter layer simply rests for
/// the remainder — the same rule a pattern's own `beats` already states for a
/// note that ends early.
///
/// **Each layer carries its own transforms, and only its own.** A solo pushed
/// up an octave does not take the groove with it, and two layers of the same
/// pattern — `"lead"` and the same lead a twelfth up — are an octave double
/// written as one line. A layer cannot itself be layered: one level is what a
/// lane in a DAW is, and a tree of them is a second arrangement inside the
/// first with nothing to show for it.
///
/// The bake report still prints **one row per entry**, named after every
/// pattern in it, because a row is a stretch of the piece and a layered entry
/// is one stretch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layers {
    /// What plays, all starting on the slot's first beat. In order, because
    /// the order is part of the seed derivation, the way a song's track order
    /// is: each note's noise is keyed by where it falls in the walk.
    pub layers: Vec<Layer>,
}

/// One pattern, and optionally how to play it — a whole arrangement entry, or
/// one of a [`Layers`] entry's layers.
///
/// Untagged, with the bare string first: a JSON string can only be a pattern
/// name and a JSON object can only be the long form, so the two never race —
/// the trick [`Pitch`](super::Pitch) and [`PatchRef`](super::PatchRef) already
/// use here.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Layer {
    /// Play the pattern as written.
    Name(String),
    /// Play it differently.
    Transformed(Play),
}

/// A pattern, and what to change about it for this one playing.
///
/// The set is closed and small, chosen for what each buys per line written.
/// Inversion, retrograde, augmentation and fragmentation are real
/// compositional operations and deliberately absent: they are the ones nobody
/// reaches for by hand, they need a pitch axis or a rhythmic grid to be defined
/// against, and getting one wrong is worse than not having it. A real song that
/// wants one is a follow-up issue with the song attached.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Play {
    /// Which pattern plays.
    pub pattern: String,
    /// Semitones added to every note, applied to the **resolved MIDI value**
    /// so it works identically for `"C#4"` and for `61.0` and a microtonal
    /// offset survives it. A note pushed past the MIDI range is clamped rather
    /// than refused: refusing would make a legal transpose depend on the
    /// register of a pattern written months earlier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transpose: Option<f32>,
    /// Scale steps added to every note **within the song's key** — the lift.
    ///
    /// The distinct one, and the distinction is the whole reason it exists.
    /// [`transpose`](Play::transpose) is chromatic: it moves everything by the
    /// same semitones, which is exactly right for an octave double and moves
    /// the music into a *different key*. `transpose_degrees: 1` moves every
    /// note one step up the scale it is already in, which is what anybody
    /// means by "lift the last chorus" — see [`Key::shift`] for what happens
    /// to a note that is not in the key.
    ///
    /// **Refused in a song with no `key`**, rather than guessed at. There is
    /// no scale to step along, and inferring one from the notes is analysis
    /// this crate does not do.
    ///
    /// **Refused beside `transpose`**, rather than composed with it. Which of
    /// the two applies first changes the answer, no chart convention decides
    /// it, and a reader should be able to tell which kind of lift an entry is
    /// at a glance. Two moves are two entries, or two tracks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transpose_degrees: Option<i32>,
    /// Multiplies every note's velocity — the dynamics dial, for a quiet
    /// reprise or a half-time breakdown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vel_scale: Option<f32>,
    /// Play only these tracks — the arrangement dial. Drop to one instrument
    /// for eight bars and a chorus arrives for free when everything comes back.
    ///
    /// A **filter, not a remap**: it names which of the song's tracks sound and
    /// cannot reassign a note to a different instrument. A remap would make a
    /// pattern mean something different depending on where it was played, and
    /// then a pattern is no longer a block of music.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tracks: Option<Vec<String>>,
}

impl ArrangementEntry {
    /// Everything that plays in this entry's slot, in walking order — one
    /// layer for every entry but a layered one.
    pub fn layers(&self) -> &[Layer] {
        match self {
            Self::Single(layer) => std::slice::from_ref(layer),
            Self::Layered(layered) => &layered.layers,
        }
    }

    /// What a report calls this entry: its pattern's name, or every layer's
    /// joined with ` + ` — `groove + solo` is the section a reader is looking
    /// for, where the first layer's name alone would say the solo is not there.
    pub fn label(&self) -> String {
        let names: Vec<&str> = self.layers().iter().map(Layer::pattern).collect();
        names.join(" + ")
    }
}

impl Layer {
    /// The pattern this layer plays.
    pub fn pattern(&self) -> &str {
        match self {
            Self::Name(name) => name,
            Self::Transformed(play) => &play.pattern,
        }
    }

    /// Semitones to add to every note, `0.0` for a bare name.
    pub fn transpose(&self) -> f32 {
        match self {
            Self::Name(_) => 0.0,
            Self::Transformed(play) => play.transpose.unwrap_or(0.0),
        }
    }

    /// Scale steps to move every note within the key, `0` for a bare name.
    pub fn transpose_degrees(&self) -> i32 {
        match self {
            Self::Name(_) => 0,
            Self::Transformed(play) => play.transpose_degrees.unwrap_or(0),
        }
    }

    /// Where a written pitch actually sounds in this playing: moved within the
    /// key, or by semitones, and clamped onto the keyboard either way.
    ///
    /// The one place both transposes are applied, so the renderer and the
    /// survey cannot come to different answers about what a section plays. A
    /// pitch pushed off the end is clamped rather than refused — whether a
    /// transpose is legal must not depend on the register of a pattern written
    /// months earlier.
    ///
    /// A keyed song runs every note through [`Key::shift`], including the
    /// notes of an entry that lifts by nothing. Skipping that for a lift of
    /// zero would be a branch around an operation that is
    /// [the identity](Key::shift) — a few integer operations saved per note,
    /// against a condition nothing could ever observe being wrong.
    pub fn played_pitch(&self, midi: f32, key: Option<&Key>) -> f32 {
        let moved = match key {
            Some(key) => key.shift(midi, self.transpose_degrees()),
            None => midi,
        };
        (moved + self.transpose()).clamp(MIDI_RANGE.0, MIDI_RANGE.1)
    }

    /// What to multiply every velocity by, `1.0` for a bare name.
    pub fn vel_scale(&self) -> f32 {
        match self {
            Self::Name(_) => 1.0,
            Self::Transformed(play) => play.vel_scale.unwrap_or(1.0),
        }
    }

    /// Whether a note on `track` sounds in this playing.
    ///
    /// A bare name and an absent `tracks` both mean everything sounds, which is
    /// what an arrangement meant before the field existed.
    pub fn plays(&self, track: &str) -> bool {
        match self {
            Self::Name(_) => true,
            Self::Transformed(play) => play
                .tracks
                .as_ref()
                .is_none_or(|only| only.iter().any(|name| name == track)),
        }
    }
}

impl From<&str> for Layer {
    fn from(name: &str) -> Self {
        Self::Name(name.to_owned())
    }
}

impl From<String> for Layer {
    fn from(name: String) -> Self {
        Self::Name(name)
    }
}

impl From<Play> for Layer {
    fn from(play: Play) -> Self {
        Self::Transformed(play)
    }
}

impl From<&str> for ArrangementEntry {
    fn from(name: &str) -> Self {
        Self::Single(name.into())
    }
}

impl From<String> for ArrangementEntry {
    fn from(name: String) -> Self {
        Self::Single(name.into())
    }
}

impl From<Play> for ArrangementEntry {
    fn from(play: Play) -> Self {
        Self::Single(play.into())
    }
}

impl From<Layer> for ArrangementEntry {
    fn from(layer: Layer) -> Self {
        Self::Single(layer)
    }
}

impl From<Layers> for ArrangementEntry {
    fn from(layered: Layers) -> Self {
        Self::Layered(layered)
    }
}
