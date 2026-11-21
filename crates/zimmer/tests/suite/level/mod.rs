//! Measuring how a sound came out.
//!
//! Every assertion here is against a **signal whose answer is known by
//! construction** — a full-scale square, a tone at one frequency, two halves
//! that differ by an exact number of decibels — because a measurement that is
//! merely self-consistent would agree with itself while being wrong, and the
//! whole value of this is that the number can be trusted against a number from
//! somewhere else.

use crate::common;

mod bands;
mod bussed;
mod defects;
mod diff;
mod integrated;
mod meter;
mod peaks;
mod sections;
mod tracks;
mod width;

/// Decibels of slack. The measurement is exact arithmetic, so this is
/// tolerance for the assertions' own rounding rather than for the meter.
pub(crate) const SLACK: f64 = 0.05;

/// A square wave of `samples` samples at `level`, at the sample rate the
/// assertion states.
///
/// A square rather than a sine because its mean and its peak are the same
/// number, so an expected level is a value written in the test rather than a
/// constant nobody can check.
pub(crate) fn square(level: f32, samples: usize) -> Vec<f32> {
    (0..samples)
        .map(|i| if i % 2 == 0 { level } else { -level })
        .collect()
}
