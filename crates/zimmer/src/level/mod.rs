//! How a finished sound came out: over its whole length, over time, and across the spectrum.

pub(crate) mod bands;
pub(crate) mod defects;
pub(crate) mod diff;
pub(crate) mod gain;
pub(crate) mod integrated;
pub(crate) mod intersample;
pub(crate) mod layer;
pub(crate) mod meter;
pub(crate) mod peaks;
pub(crate) mod profile;

pub use bands::{BandMeter, Bands};
pub use defects::{Defect, DefectFinder, Defects, Tally, WORST};
pub use diff::{BandsDifference, Difference};
pub use gain::{db_to_gain, db_to_gain_f32, gain_to_db, gain_to_db_f32};
pub use integrated::Integrated;
pub use layer::Layer;
pub use meter::{Loudness, Meter};
pub use peaks::FramePeaks;
pub use profile::{Cut, Profile, Profiler, Span};
