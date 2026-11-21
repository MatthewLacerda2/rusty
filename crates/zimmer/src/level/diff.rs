//! Two measurements, compared field by field.
//!
//! **This is the part with teeth.** An absolute number is hard to judge — is
//! −14 dBFS good? It depends entirely on what it is, what is under it, and what
//! the piece is for. A difference is not: "4.6 dB under the version it was meant
//! to replace" is a finding, and it is the only form in which two of the three
//! defects that argued for this whole module were ever visible.
//!
//! It is also the form an agent iterating on a score actually needs. Rewriting a
//! recipe and re-baking answers "did the change land?" only if the two bakes can
//! be put side by side; without that, the agent is left asking the author to
//! listen again, which is the round trip this exists to remove.
//!
//! Every field is a plain subtraction, `this` minus `that`. Decibels subtract to
//! decibels; band shares subtract to **percentage points**, which is a different
//! unit from a percentage and is worth saying out loud — a band going from 10%
//! to 20% is ten points and twice the energy, and only one of those two numbers
//! is the one a report should print.

use super::bands::Bands;
use super::profile::Span;

/// How one measurement differs from another.
///
/// A field is `None` when either side had nothing to compare — a silence has no
/// level and no spectral balance, and reporting a difference against it would be
/// inventing one. `None` is not zero: zero says the two matched.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Difference {
    /// Mean level, in decibels. Negative means quieter than what it is compared
    /// against.
    pub mean_db: Option<f64>,
    /// Sample peak, in decibels.
    pub peak_db: Option<f64>,
    /// Crest factor, in decibels. Negative means flatter — less room between the
    /// loudest instant and the average one.
    pub crest_db: Option<f64>,
    /// Band shares, in percentage points.
    pub bands: Option<BandsDifference>,
    /// Width, as a plain subtraction in the same `-1..=1` units the
    /// correlation itself is in.
    ///
    /// **Positive means narrower** — more of the signal common to both
    /// channels — and it is the one field here whose sign a reader will guess
    /// backwards, since every other one goes up when there is more of
    /// something. Worth saying out loud wherever this is printed.
    pub correlation: Option<f64>,
    /// Running time, in seconds.
    pub seconds: f64,
}

/// How two spectral balances differ, in percentage points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BandsDifference {
    /// Points of energy gained or lost below the low crossover.
    pub low: f64,
    /// Points across the midrange.
    pub mid: f64,
    /// Points above the high crossover.
    pub high: f64,
}

impl Difference {
    /// How `this` differs from `that`.
    pub fn between(this: &Span, that: &Span) -> Self {
        Self {
            mean_db: subtract(this.loudness.mean_dbfs, that.loudness.mean_dbfs),
            peak_db: subtract(this.loudness.peak_dbfs, that.loudness.peak_dbfs),
            crest_db: subtract(this.crest_db(), that.crest_db()),
            bands: match (this.bands, that.bands) {
                (Some(this), Some(that)) => Some(BandsDifference::between(&this, &that)),
                _ => None,
            },
            correlation: subtract(this.correlation, that.correlation),
            seconds: this.seconds() - that.seconds(),
        }
    }

    /// True when nothing measurably changed.
    ///
    /// The threshold is deliberately below anything anyone can hear: this
    /// answers "is this the same audio?" and not "is this close enough?", and a
    /// comparison mode that shrugged at half a decibel would hide the exact
    /// defect it was built to find. A file compared with itself is the case that
    /// has to come out true.
    pub fn is_nothing(&self) -> bool {
        let settled = |field: Option<f64>| settled_by(field, NOTHING_DB);
        settled(self.mean_db)
            && settled(self.peak_db)
            && settled(self.crest_db)
            && self
                .bands
                .is_none_or(|bands| bands.largest() < NOTHING_POINTS)
            && settled_by(self.correlation, NOTHING_CORRELATION)
            && self.seconds.abs() < NOTHING_SECONDS
    }
}

impl BandsDifference {
    /// How `this` differs from `that`, in percentage points.
    pub fn between(this: &Bands, that: &Bands) -> Self {
        Self {
            low: (this.low - that.low) * 100.0,
            mid: (this.mid - that.mid) * 100.0,
            high: (this.high - that.high) * 100.0,
        }
    }

    /// The largest move any one band made, in points.
    pub fn largest(&self) -> f64 {
        self.low.abs().max(self.mid.abs()).max(self.high.abs())
    }
}

/// Below this many decibels, two levels are the same level.
const NOTHING_DB: f64 = 0.01;

/// Below this many percentage points, two balances are the same balance.
const NOTHING_POINTS: f64 = 0.05;

/// Below this much correlation, two widths are the same width. A quarter of a
/// percent of a range two units wide, which is the same claim the decibel
/// threshold above makes: this answers "is this the same audio?" and not "is
/// this close enough?".
const NOTHING_CORRELATION: f64 = 0.005;

/// Below this many seconds, two lengths are the same length.
const NOTHING_SECONDS: f64 = 0.001;

/// True when a field moved by less than `threshold`, or did not exist to move.
fn settled_by(field: Option<f64>, threshold: f64) -> bool {
    field.is_none_or(|by| by.abs() < threshold)
}

/// A difference, or nothing when either side is missing.
fn subtract(this: Option<f64>, that: Option<f64>) -> Option<f64> {
    Some(this? - that?)
}
