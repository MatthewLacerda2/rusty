//! The patch document: one instrument, as serde data.
//!
//! A [`Patch`] is a "Minimoog-lite" description of a single sound. It is
//! **structured, not a free graph**: the signal path is fixed —
//! `source → filter → amp envelope → fx`, with up to two LFOs tapping one target each ([`Lfo`]), and
//! up to [`MAX_LAYERS`] more `source → filter → amp` paths summed in parallel
//! before the fx ([`Layer`]) — because a patch has to honour a contract, *playable as a note*. The recipe
//! chooses what goes in each stage, never how the stages connect.
//!
//! That is not a simplification for its own sake. A free graph would let a
//! recipe describe something that is not a note — a feedback loop with no
//! output, a stage that never terminates — and the whole point of a patch is
//! that `(pitch, velocity, duration) in → buffer out` always holds.
//!
//! Patch-as-truth: no buffers and no handles, so it round-trips losslessly.
//! [`render_note`](crate::render_note) renders it.

pub(crate) mod fm;
pub(crate) mod fx;
mod layer;
mod lfo;
pub(crate) mod stages;
mod vary;

use serde::{Deserialize, Serialize};

use crate::error::SynthError;

pub use fm::{Algorithm, FM_OPERATORS, Operator};
pub use fx::{EqBand, EqKind, Fx, MAX_EQ_BANDS};
pub use layer::{Layer, MAX_LAYERS};
pub use lfo::{Lfo, LfoShape, LfoTarget, MAX_LFOS};
pub use stages::{
    Adsr, Filter, FilterKind, MAX_OSCS, MAX_PARTIALS, MAX_PULSE_WIDTH, MAX_VOICES, MIN_PULSE_WIDTH,
    NoiseColor, Osc, Partial, PitchEnv, Slope, Source, Wave,
};

/// Rejects an fx chain the renderer cannot honour.
///
/// It lives beside the patch rather than inside it because a chain lives in
/// **three** places — a patch, a track and the song — and one check for all
/// three is what stops the cap being a property of where the chain happens to
/// be written. The song's own validation calls this for the other two.
pub(crate) fn check_chain(chain: &[Fx]) -> Result<(), SynthError> {
    for fx in chain {
        match fx {
            Fx::Eq { bands } if bands.len() > MAX_EQ_BANDS => {
                return Err(SynthError::TooManyEqBands {
                    found: bands.len(),
                    limit: MAX_EQ_BANDS,
                });
            }
            Fx::Delay {
                time,
                beats: Some(beats),
                ..
            } if *time != 0.0 => {
                return Err(SynthError::DelayTwoTimes {
                    time: *time,
                    beats: *beats,
                });
            }
            _ => {}
        }
    }
    Ok(())
}

/// Rejects a delay timed in `beats` on a patch's chain, which plays one note
/// and has no tempo to count them by — refused, like a `sidechain` there,
/// rather than played as no delay at all.
fn check_no_beats(chain: &[Fx]) -> Result<(), SynthError> {
    for fx in chain {
        if let Fx::Delay {
            beats: Some(beats), ..
        } = fx
        {
            return Err(SynthError::MisplacedBeats {
                place: "patch",
                beats: *beats,
            });
        }
    }
    Ok(())
}

/// Every track a chain is keyed from, in list order.
///
/// It lives beside the patch for the reason [`check_chain`] does: a chain is a
/// chain wherever it is written, and both the mixer that honours a sidechain
/// and the validation that refuses one read it through this.
pub(crate) fn sidechains(chain: &[Fx]) -> impl Iterator<Item = &str> {
    chain.iter().filter_map(|fx| match fx {
        Fx::Compress { sidechain, .. } => sidechain.as_deref(),
        _ => None,
    })
}

/// Rejects a chain that names a track from somewhere no track can be named.
///
/// A patch's chain runs per note and the song's runs on the sum; in neither
/// does a track exist as a thing to listen to. Refusing is the crate's usual
/// answer to what it cannot honour — a silently ignored `sidechain` would be a
/// duck the recipe wrote and never got, which is exactly the failure the song
/// validator exists to prevent.
pub(crate) fn check_no_sidechain(chain: &[Fx], place: &'static str) -> Result<(), SynthError> {
    match sidechains(chain).next() {
        Some(key) => Err(SynthError::MisplacedSidechain {
            place,
            key: key.to_owned(),
        }),
        None => Ok(()),
    }
}

/// One instrument.
///
/// `source` and `amp` are mandatory — a sound needs a tone and a shape.
/// `filter`, `lfo` and `fx` are optional stages.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Patch {
    /// What makes the raw tone.
    pub source: Source,
    /// The amplitude envelope: what turns a continuous tone into a note.
    pub amp: Adsr,
    /// The optional filter stage, with its own envelope.
    #[serde(default)]
    pub filter: Option<Filter>,
    /// An optional one-shot sweep of the note's own pitch. Absent means the
    /// note holds the pitch it was played at, which is what every patch
    /// written before this stage existed already meant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pitch_env: Option<PitchEnv>,
    /// More sounds in the same note, each its own `source → filter → amp`,
    /// added to the one above before `fx`. Empty means the patch is the one
    /// sound written above, which is what every patch before layers meant.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layers: Vec<Layer>,
    /// The optional low-frequency oscillator, bending one stage of every
    /// layer.
    #[serde(default)]
    pub lfo: Option<Lfo>,
    /// A second LFO, for a patch that moves two ways at once — a pad that
    /// breathes and shimmers. Aimed at the same target as `lfo`, the two add.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lfo2: Option<Lfo>,
    /// The post-chain, applied in list order.
    #[serde(default)]
    pub fx: Vec<Fx>,
}

impl Patch {
    /// Serialises to pretty JSON — the on-disk recipe form.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Parses a patch from its JSON form.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Rejects a patch the renderer cannot honour, *before* any samples are
    /// produced.
    ///
    /// Only the bounds that would produce silence, a divide-by-zero or an
    /// unstable filter are checked. Musical taste is the recipe's business: an
    /// ugly patch renders.
    ///
    /// This is the patch played on its own, as a one-shot, which has no tempo:
    /// an LFO timed in `beats` is refused here and accepted by
    /// [`Patch::validate_in_song`].
    pub fn validate(&self) -> Result<(), SynthError> {
        self.check(false)
    }

    /// [`Patch::validate`] for a patch a song plays, where an LFO may be timed
    /// in `beats` of the song.
    pub fn validate_in_song(&self) -> Result<(), SynthError> {
        self.check(true)
    }

    /// The LFOs the patch carries, `lfo` first.
    pub(crate) fn lfos(&self) -> impl Iterator<Item = &Lfo> {
        self.lfo.iter().chain(&self.lfo2)
    }

    fn check(&self, timed: bool) -> Result<(), SynthError> {
        check_stages(&self.source, self.filter.as_ref())?;
        layer::check_layers(&self.layers)?;
        for lfo in self.lfos() {
            lfo.check(timed)?;
        }
        self.check_pulse()?;
        check_chain(&self.fx)?;
        check_no_beats(&self.fx)?;
        check_no_sidechain(&self.fx, "patch")
    }

    /// An LFO aimed at the pulse width of a patch with no square in any of
    /// its sounds, top level or layer, to move. A width written on a wave
    /// other than a square is [`check_source`]'s, so a layer's is named by
    /// its index.
    fn check_pulse(&self) -> Result<(), SynthError> {
        let squared = std::iter::once(&self.source)
            .chain(self.layers.iter().map(|layer| &layer.source))
            .flat_map(oscillators)
            .any(|osc| osc.wave == Wave::Square);
        if !squared && self.lfos().any(|lfo| lfo.target == LfoTarget::PulseWidth) {
            return Err(SynthError::NoPulseToModulate);
        }
        Ok(())
    }
}

/// The oscillators of a source: a stack's, and none for any other kind.
fn oscillators(source: &Source) -> &[Osc] {
    match source {
        Source::OscStack { oscs } => oscs,
        _ => &[],
    }
}

/// The two stages a patch and each of its layers carry and can get wrong: the
/// source and the filter's cutoff.
fn check_stages(source: &Source, filter: Option<&Filter>) -> Result<(), SynthError> {
    check_source(source)?;
    match filter {
        Some(filter) if filter.cutoff <= 0.0 => Err(SynthError::BadCutoff {
            cutoff: filter.cutoff,
        }),
        _ => Ok(()),
    }
}

/// The head of the signal path, which is where every unrenderable patch so
/// far has gone wrong.
fn check_source(source: &Source) -> Result<(), SynthError> {
    if let Some(osc) = oscillators(source)
        .iter()
        .find(|osc| osc.wave != Wave::Square && osc.pulse_width != 0.5)
    {
        return Err(SynthError::PulseWidthOffSquare { wave: osc.wave });
    }
    match source {
        Source::OscStack { oscs } if oscs.is_empty() => Err(SynthError::EmptyOscStack),
        Source::OscStack { oscs } if oscs.len() > MAX_OSCS => Err(SynthError::TooManyOscs {
            found: oscs.len(),
            limit: MAX_OSCS,
        }),
        Source::OscStack { oscs } if oscs.iter().all(|osc| osc.gain <= 0.0) => {
            Err(SynthError::SilentOscStack)
        }
        // Both ends of the unison range, in one pass: no voices at all is
        // an oscillator the recipe asked to be silent without saying so,
        // and too many is [`MAX_VOICES`]'s argument.
        Source::OscStack { oscs } => match oscs
            .iter()
            .find(|osc| !(1..=MAX_VOICES).contains(&osc.voices))
        {
            Some(osc) => Err(SynthError::BadVoiceCount {
                found: osc.voices,
                limit: MAX_VOICES,
            }),
            None => Ok(()),
        },
        Source::Fm2 { ratio, .. } if *ratio <= 0.0 => Err(SynthError::BadFmRatio { ratio: *ratio }),
        Source::Fm4 {
            algorithm,
            operators,
            ..
        } => check_operators(*algorithm, operators),
        Source::Additive { partials } => check_partials(partials),
        _ => Ok(()),
    }
}

/// Rejects an additive series the renderer cannot honour.
///
/// The same four questions `check_source` asks of an oscillator stack — is
/// there anything in it, is there too much of it, does each entry name a
/// frequency, and does any of it sound — because a series is the same kind of
/// table with the same ways of being wrong.
///
/// **Not asked: whether a partial is above Nyquist.** That depends on the
/// pitch played, and a patch is validated once while it is played at many
/// pitches, so refusing here would refuse a legal organ for the top note of a
/// part. The renderer drops those per note instead.
fn check_partials(partials: &[Partial]) -> Result<(), SynthError> {
    if partials.is_empty() {
        return Err(SynthError::EmptyPartials);
    }
    if partials.len() > MAX_PARTIALS {
        return Err(SynthError::TooManyPartials {
            found: partials.len(),
            limit: MAX_PARTIALS,
        });
    }
    if let Some((index, partial)) = partials
        .iter()
        .enumerate()
        .find(|(_, partial)| partial.ratio <= 0.0 || !partial.ratio.is_finite())
    {
        return Err(SynthError::BadPartialRatio {
            index,
            ratio: partial.ratio,
        });
    }
    if partials.iter().all(|partial| partial.gain <= 0.0) {
        return Err(SynthError::SilentPartials);
    }
    Ok(())
}

/// Rejects a four-operator source the renderer cannot honour.
///
/// Two of the four questions [`check_partials`] asks, because the shape of the
/// document has already answered the other two: an `fm4` source carries
/// exactly [`FM_OPERATORS`] operators as a fixed-size array, so it can be
/// neither empty nor oversized by the time serde has finished with it.
///
/// What is left is the same pair. Does each entry name a frequency — the
/// [`SynthError::BadPartialRatio`] question, and refusing a non-finite one for
/// the same reason. And does any of it sound: the
/// [`SynthError::SilentOscStack`] question, one indirection further away,
/// because which operators are audible is a property of the *algorithm* rather
/// than of the list. A recipe that picks [`Algorithm::Chain`] and then writes
/// its levels as if they were faders silences the whole source by zeroing the
/// one operator it happens to be heard through, with nothing on the page to
/// say so.
///
/// **Not asked: whether an operator is above Nyquist** — the same answer
/// [`check_partials`] gives, for the same reason. That depends on the pitch
/// played, and a patch is validated once while it is played at many pitches.
/// The renderer drops those per note instead.
fn check_operators(
    algorithm: Algorithm,
    operators: &[Operator; FM_OPERATORS],
) -> Result<(), SynthError> {
    if let Some((index, operator)) = operators
        .iter()
        .enumerate()
        .find(|(_, operator)| operator.ratio <= 0.0 || !operator.ratio.is_finite())
    {
        return Err(SynthError::BadOperatorRatio {
            // Numbered from **one**, unlike a partial's index, and the field
            // name says which: a partial's is its place in a list whose order
            // carries no meaning, while an operator's number is part of the
            // algorithm's own vocabulary. An error reading "operator 0" would
            // send a reader looking for a row that is not on the page.
            operator: index + 1,
            ratio: operator.ratio,
        });
    }
    if (0..FM_OPERATORS)
        .filter(|index| algorithm.is_carrier(*index))
        .all(|index| operators[index].level <= 0.0)
    {
        return Err(SynthError::SilentCarriers {
            algorithm: algorithm.name(),
        });
    }
    Ok(())
}
