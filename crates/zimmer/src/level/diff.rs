//! Two measurements, compared field by field.

use super::bands::Bands;
use super::profile::Span;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Difference {
    pub mean_db: Option<f64>,
    pub peak_db: Option<f64>,
    pub crest_db: Option<f64>,
    pub bands: Option<BandsDifference>,
    pub correlation: Option<f64>,
    pub seconds: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BandsDifference {
    pub low: f64,
    pub mid: f64,
    pub high: f64,
}

impl Difference {
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
    pub fn between(this: &Bands, that: &Bands) -> Self {
        Self {
            low: (this.low - that.low) * 100.0,
            mid: (this.mid - that.mid) * 100.0,
            high: (this.high - that.high) * 100.0,
        }
    }

    pub fn largest(&self) -> f64 {
        self.low.abs().max(self.mid.abs()).max(self.high.abs())
    }
}

// "The same audio", not "close enough", below anything audible: dB, percentage points,
// correlation and seconds.
const NOTHING_DB: f64 = 0.01;

const NOTHING_POINTS: f64 = 0.05;

const NOTHING_CORRELATION: f64 = 0.005;

const NOTHING_SECONDS: f64 = 0.001;

fn settled_by(field: Option<f64>, threshold: f64) -> bool {
    field.is_none_or(|by| by.abs() < threshold)
}

fn subtract(this: Option<f64>, that: Option<f64>) -> Option<f64> {
    Some(this? - that?)
}
