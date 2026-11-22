//! The patch document: one instrument, as serde data.

pub(crate) mod fm;
pub(crate) mod fx;
mod layer;
mod lfo;
pub(crate) mod stages;
mod vary;

use serde::{Deserialize, Serialize};

use crate::error::SynthError;

pub use fm::{Algorithm, Operator, FM_OPERATORS};
pub use fx::{EqBand, EqKind, Fx, MAX_EQ_BANDS};
pub use layer::{Layer, MAX_LAYERS};
pub use lfo::{Lfo, LfoShape, LfoTarget, MAX_LFOS};
pub use stages::{
    Adsr, Filter, FilterKind, NoiseColor, Osc, Partial, PitchEnv, Slope, Source, Wave, MAX_OSCS,
    MAX_PARTIALS, MAX_PULSE_WIDTH, MAX_VOICES, MIN_PULSE_WIDTH,
};

/// What every fx chain is checked for: `Patch::check` runs it on the patch's own, `song::validate`
/// on each track's and the song's.
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

/// The keys `song::validate` checks and `song::render::mix` honours: both read them here.
pub(crate) fn sidechains(chain: &[Fx]) -> impl Iterator<Item = &str> {
    chain.iter().filter_map(|fx| match fx {
        Fx::Compress { sidechain, .. } => sidechain.as_deref(),
        _ => None,
    })
}

pub(crate) fn check_no_sidechain(chain: &[Fx], place: &'static str) -> Result<(), SynthError> {
    match sidechains(chain).next() {
        Some(key) => Err(SynthError::MisplacedSidechain {
            place,
            key: key.to_owned(),
        }),
        None => Ok(()),
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Patch {
    pub source: Source,
    pub amp: Adsr,
    #[serde(default)]
    pub filter: Option<Filter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pitch_env: Option<PitchEnv>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layers: Vec<Layer>,
    #[serde(default)]
    pub lfo: Option<Lfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lfo2: Option<Lfo>,
    #[serde(default)]
    pub fx: Vec<Fx>,
}

impl Patch {
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    pub fn validate(&self) -> Result<(), SynthError> {
        self.check(false)
    }

    pub fn validate_in_song(&self) -> Result<(), SynthError> {
        self.check(true)
    }

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

fn oscillators(source: &Source) -> &[Osc] {
    match source {
        Source::OscStack { oscs } => oscs,
        _ => &[],
    }
}

fn check_stages(source: &Source, filter: Option<&Filter>) -> Result<(), SynthError> {
    check_source(source)?;
    match filter {
        Some(filter) if filter.cutoff <= 0.0 => Err(SynthError::BadCutoff {
            cutoff: filter.cutoff,
        }),
        _ => Ok(()),
    }
}

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

// Neither this nor `check_operators` refuses a ratio above Nyquist: that depends on the pitch
// played, so `core` drops such a partial or operator per note.
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
            // From one, as the recipe and the algorithm table number operators; a partial's
            // index is its place in a list, from zero.
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
