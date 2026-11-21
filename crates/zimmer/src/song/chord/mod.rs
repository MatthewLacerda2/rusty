//! A chord: one entry in a pattern that sounds as several notes.
//!
//! Before this, a chord was three or four [`Note`]s with hand-computed
//! pitches, and the document said nothing about their being one thing. That is
//! **coupling with no mechanism**: transposing the chord is four coordinated
//! edits, changing its quality is four more, and any one of them can disagree
//! with the others — producing a chord that is not wrong enough to be refused
//! and not right enough to be what was meant. Nothing catches that; only ears
//! find it.
//!
//! So a chord is one entry, and it expands to notes **before** anything else
//! in the renderer touches them. Each voice is an ordinary note from that
//! point on: it gets its own swing displacement, its own humanise nudge and its
//! own render seed, which is why a chord written this way does not land as one
//! perfectly simultaneous block. That is not a bonus feature, it is what a
//! played chord sounds like, and it comes free from putting expansion first.
//!
//! ## The voicing rule, and why it is the boring one
//!
//! `Dm7` names four pitch classes and not which octave each sits in, and the
//! difference between a good voicing and a bad one is most of what a chord
//! sounds like. This module picks **close position from the root**: the root at
//! `oct`, then each remaining chord tone at the next semitone offset above it,
//! nothing dropped, nothing spread. `Dm7` at octave 3 is `D3 F3 A3 C4`.
//!
//! Three reasons, in order of how much they weigh:
//!
//! 1. **It is the voicing the name literally describes.** A reader works it out
//!    in their head, and a document whose sound cannot be predicted from
//!    reading it is not a document.
//! 2. **It re-qualifies and transposes without surprise.** `Dm7` to `D7` moves
//!    exactly one voice; a transpose moves all of them by the same amount. A
//!    cleverer default — drop-2, spread, voice-leading between neighbours —
//!    would make each of those edits move notes the writer did not ask about.
//! 3. **Anything cleverer is taste**, and taste is a property *value*. This
//!    crate defines property types and leaves values to the document, the same
//!    rule that lets a recipe choose a colour and forbids the code choosing
//!    red.
//!
//! Which leaves the escape hatch, and it has to exist because a real
//! arrangement will want a voicing no name spells. `chord` therefore takes
//! **either a name or the pitches themselves** — untagged, told apart by a JSON
//! string against a JSON array, the trick [`Pitch`] already uses. Spelling the
//! pitches keeps the one thing the name form was for: the voices are still one
//! entry, with one `start`, one `dur` and one `vel` between them.
//!
//! A `/bass` in the name covers the common inversion cheaply — see
//! [`name`] for the grammar and for what it refuses.
//!
//! ## Asking for something else
//!
//! The boring voicing is the default, not the only one. A chord that says so —
//! [`inv`](Chord::inv) for a fixed inversion, or
//! [`voicing: "near"`](Chord::voicing) for the inversion closest to the chord
//! before it — gets the voicing it asked for, and the reasons above still hold
//! for every chord that does not ask. Reason 2 is why `near` is opt-in: it is
//! the one rule that makes editing one chord move the notes of the next.
//! [`lead`] has both.
//!
//! ## Stated, or played
//!
//! Everything above is a chord *stated*: every voice at once, for the length of
//! the entry. An [`arp`](Chord::arp) plays the same voices one at a time
//! instead — a figure that repeats until the chord is over. It is the same
//! entry, the same pitches and the same grammar; only the order and the onsets
//! change, and [`arp`] has the whole of it.

pub(crate) mod arp;
pub(crate) mod lead;
pub(crate) mod name;

use serde::{Deserialize, Serialize};

pub use arp::Arp;
pub use lead::Leading;
pub(crate) use lead::Led;

use super::{Articulation, Note, Pitch, one};
use crate::error::SynthError;
use crate::note::MIDI_RANGE;

/// Where a chord's root sits when the document does not say.
///
/// Octave 3 puts a close-position seventh chord across middle C — the register
/// a pair of hands actually plays chords in, and low enough that a ninth on top
/// still has somewhere to go.
const DEFAULT_OCT: i32 = 3;

/// Several notes as one idea: which track plays them, what they are, when, for
/// how long, how hard.
///
/// Every field but `chord` and `oct` means exactly what it means on a
/// [`Note`], because every voice becomes one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Chord {
    /// The [`Track::name`](super::Track::name) that plays it.
    pub track: String,
    /// What to play: a name, or the pitches spelled out.
    pub chord: Voicing,
    /// Which octave the **root** sits in, in scientific pitch notation, so
    /// `"Dm7"` at `3` is rooted on `D3`. Absent means octave 3, which puts a
    /// close-position seventh chord across middle C.
    ///
    /// Meaningless — and refused — beside a chord spelled as pitches, which
    /// already carries an octave per voice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oct: Option<i32>,
    /// Onset in beats from the start of its pattern.
    pub start: f32,
    /// How long the chord lasts, in beats. Stated as a block that is every
    /// voice's gate; [arpeggiated](Self::arp) it is the slot the figure fills,
    /// and each note's gate is [`gate`](Self::gate) instead. The patch's
    /// release rings out after either.
    pub dur: f32,
    /// Velocity in `0..=1` for every voice, before the performance scatters
    /// them apart.
    #[serde(default = "one")]
    pub vel: f32,
    /// How every voice is played — see [`Note::articulation`]. A chord is one
    /// gesture of a hand, so an articulation belongs to the whole of it rather
    /// than to one of its voices; a voice that is played differently from the
    /// others is a note beside the chord, not a chord with an exception in it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub articulation: Option<Articulation>,
    /// Play the voices one at a time, in this order, rather than all at once —
    /// see [`Arp`] for the three words and what each of them walks. Absent is a
    /// block chord, which is what every chord written before this field existed
    /// was.
    ///
    /// It orders the voices this entry already has and never invents one, so an
    /// arpeggio is notation rather than a generator.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arp: Option<Arp>,
    /// How long one step of the figure is, in beats: `0.5` an eighth, `0.25` a
    /// sixteenth. Required beside an [`arp`](Self::arp) — no default is right,
    /// because it is the number that decides what the figure *is* — and
    /// meaningless without one, so it is refused there rather than ignored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub div: Option<f32>,
    /// Gate length in beats for every note of the figure. Absent means one
    /// step, which is what a step sequencer means by a step and what
    /// [`Steps`](super::Steps) already means by an absent `dur`; a longer gate
    /// is how an arpeggio on a sustaining patch accumulates into its chord.
    /// Meaningless without an [`arp`](Self::arp) — a block chord's gate is its
    /// `dur` — and refused there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gate: Option<f32>,
    /// Which inversion of a named chord to voice: its lowest voice raised an
    /// octave this many times, so `"C"` with `1` is `E G C`. A slash bass stays
    /// underneath and only the voices above it turn. Absent is root position.
    ///
    /// Up to one fewer than the voices above the bass — the next is root
    /// position an octave up, which is [`oct`](Self::oct)'s job — and refused
    /// beside spelled pitches or beside [`voicing`](Self::voicing).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inv: Option<u8>,
    /// Choose the voicing from the chord before: `"near"` takes the inversion,
    /// within an octave of root position, whose voices move least from the
    /// previous chord on the same track in the same pattern. Absent is root
    /// position (or [`inv`](Self::inv)); see [`Leading`]. Refused beside spelled
    /// pitches, which are already a voicing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voicing: Option<Leading>,
}

/// What a chord is: a name for the grammar to read, or the pitches themselves.
///
/// Untagged, and `Name` first: a JSON string can only be a name and a JSON
/// array can only be a list of pitches, so the two never race.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Voicing {
    /// A chord name — `"Dm7"`, `"F#maj9"`, `"C/G"`. Voiced close position from
    /// the root at [`Chord::oct`]; see the module doc for the grammar and for
    /// why that default and not a cleverer one.
    Name(String),
    /// The pitches themselves, low to high, for a voicing no name spells.
    Pitches(Vec<Pitch>),
}

impl Chord {
    /// Appends this chord's voices to `out`, or says why it cannot, and
    /// records them in `led` as the last chord on its track.
    ///
    /// Appends rather than returns, so a pattern's voices are built into one
    /// allocation and the ordinals the renderer hands out stay in document
    /// order. Nothing is appended, and nothing recorded, when the chord is
    /// refused.
    pub(crate) fn voice_into(&self, led: &mut Led, out: &mut Vec<Note>) -> Result<(), SynthError> {
        let voices = match &self.chord {
            Voicing::Name(name) => self.named(name, led)?,
            Voicing::Pitches(pitches) => self.spelled(pitches)?,
        };
        // Here rather than beside the spelling that can produce one, because an
        // arpeggio walks whatever this is and "no voices" has to be a refusal
        // before anything walks it.
        if voices.is_empty() {
            return Err(SynthError::EmptyChord {
                track: self.track.clone(),
                start: self.start,
            });
        }
        match self.arp {
            Some(arp) => arp::spread(self, arp, &voices, out)?,
            None => {
                self.check_no_arp_fields()?;
                out.extend(voices.iter().cloned());
            }
        }
        led.record(&self.track, &voices);
        Ok(())
    }

    /// A [`SynthError::BadVoicing`] on this chord, saying `why`.
    fn bad_voicing(&self, why: &'static str) -> SynthError {
        SynthError::BadVoicing {
            track: self.track.clone(),
            start: self.start,
            why,
        }
    }

    /// That `div` and `gate` are not written on a chord that is not
    /// arpeggiated.
    ///
    /// Refused rather than ignored, for the reason `oct` beside spelled
    /// pitches is: a field the entry cannot act on is a sentence the writer
    /// believes they wrote, and silence is the worst answer to it.
    fn check_no_arp_fields(&self) -> Result<(), SynthError> {
        let refuse = |why| {
            Err(SynthError::BadArp {
                track: self.track.clone(),
                start: self.start,
                why,
            })
        };
        if self.div.is_some() {
            return refuse("`div` is the step of a figure, so it means nothing without an `arp`");
        }
        if self.gate.is_some() {
            return refuse(
                "`gate` is the length of one note of a figure, so it means nothing without an `arp` — a block chord's gate is its `dur`",
            );
        }
        Ok(())
    }

    /// The voices of a named chord: close position from its root at `oct`,
    /// then turned as [`inv`](Self::inv) or [`voicing`](Self::voicing) asks.
    ///
    /// Root position must be on the keyboard whichever voicing is chosen, so
    /// `oct` means the same thing on every chord and a `near` chord is refused
    /// for the same documents with or without a chord before it.
    fn named(&self, name: &str, led: &Led) -> Result<Vec<Note>, SynthError> {
        let oct = self.oct.unwrap_or(DEFAULT_OCT);
        let spelled = name::spell(name)?;
        let root = (oct + 1) * 12 + spelled.root;
        let range = MIDI_RANGE.0 as i32..=MIDI_RANGE.1 as i32;
        let on_keys = |midis: &[i32]| match midis.iter().find(|midi| !range.contains(midi)) {
            Some(&midi) => Err(SynthError::ChordOutOfRange {
                chord: name.to_owned(),
                oct,
                midi,
            }),
            None => Ok(()),
        };
        let (bass, upper): (Vec<i32>, Vec<i32>) = spelled
            .offsets
            .iter()
            .map(|offset| root + offset)
            .partition(|&midi| midi < root);
        let stack = lead::Stack {
            bass: bass.first().copied(),
            upper,
        };
        on_keys(&stack.turned(0))?;
        let midis = match (self.inv, self.voicing) {
            (Some(_), Some(_)) => {
                return Err(self.bad_voicing(
                    "`inv` and `voicing` both choose the inversion — write one of them",
                ));
            }
            (Some(inv), None) => {
                if usize::from(inv) >= stack.upper.len() {
                    return Err(self.bad_voicing(
                        "`inv` is past this chord's last inversion, which is one fewer than its voices above any slash bass — the next is root position an octave up, which is a higher `oct`",
                    ));
                }
                stack.turned(i32::from(inv))
            }
            (None, Some(Leading::Near)) => match led.last(&self.track) {
                Some(previous) => stack.nearest(previous, &range),
                None => stack.turned(0),
            },
            (None, None) => stack.turned(0),
        };
        on_keys(&midis)?;
        Ok(midis
            .into_iter()
            .map(|midi| self.voice(Pitch::Midi(midi as f32)))
            .collect())
    }

    /// The voices of a chord written as pitches, which are the pitches.
    fn spelled(&self, pitches: &[Pitch]) -> Result<Vec<Note>, SynthError> {
        if let Some(oct) = self.oct {
            return Err(SynthError::SpelledChordOctave {
                track: self.track.clone(),
                start: self.start,
                oct,
            });
        }
        if self.inv.is_some() || self.voicing.is_some() {
            return Err(self.bad_voicing(
                "`inv` and `voicing` choose how a named chord is voiced, and spelled pitches are already a voicing",
            ));
        }
        pitches
            .iter()
            // Resolved here so a pitch that does not parse is reported against
            // the chord holding it rather than at the sample loop.
            .map(|pitch| pitch.to_midi().map(|_| self.voice(pitch.clone())))
            .collect()
    }

    /// One voice of this chord: the chord's timing, force and articulation,
    /// one pitch.
    fn voice(&self, note: Pitch) -> Note {
        Note {
            track: self.track.clone(),
            note,
            start: self.start,
            dur: self.dur,
            vel: self.vel,
            articulation: self.articulation,
        }
    }
}

#[cfg(test)]
mod tests;
