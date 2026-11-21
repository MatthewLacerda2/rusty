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

pub(crate) const SLACK: f64 = 0.05;

pub(crate) fn square(level: f32, samples: usize) -> Vec<f32> {
    (0..samples)
        .map(|i| if i % 2 == 0 { level } else { -level })
        .collect()
}
