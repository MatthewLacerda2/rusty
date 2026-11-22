//! Measuring how a sound came out.

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

/// Decibels: the meters are exact arithmetic, so this is for the assertions' own rounding.
pub(crate) const SLACK: f64 = 0.05;

/// A square, whose mean and peak are one number, so an expected level is written in the test.
pub(crate) fn square(level: f32, samples: usize) -> Vec<f32> {
    (0..samples)
        .map(|i| if i % 2 == 0 { level } else { -level })
        .collect()
}
