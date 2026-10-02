//! src/audio/reverb/ — reverb zones: the listener's blended reverb (#469).
//!
//! Unity's `AudioReverbZone` model. Every zone is a sphere with a full-effect
//! radius (`min_distance`) and a fade-out radius (`max_distance`); the listener's
//! weight in a zone is 1 inside the first, 0 past the second and linear between.
//! Where zones overlap, the reverb's character (decay, pre-delay, damping) is the
//! weight-averaged character of the zones the listener is in, and its level (`wet`)
//! is their weighted sum over `max(total weight, 1)` — so a lone zone fades to dry
//! at its edge, and two full-weight zones average instead of doubling.
//!
//! This is the sim-side half: a pure function of the listener's position and the
//! zones, resolved each `LateUpdate` (`app/audio.rs`) and readable headlessly
//! (`Audio.GetReverbState`). The device renders it on the reverb bus every mixer
//! group sends into (`device/reverb.rs`).

mod maestro;

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::components::{ReverbParams, ReverbZoneComponent};

/// The reverb the listener hears, and how much of it comes from zones.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReverbState {
    /// The blended params the reverb bus runs at.
    pub params: ReverbParams,
    /// The summed zone weight at the listener, capped at 1 (`0` = in no zone).
    pub weight: f32,
    /// How many zones reach the listener (weight above 0).
    pub zones: usize,
}

impl ReverbState {
    /// Outside every zone: dry.
    pub const DRY: ReverbState = ReverbState {
        params: ReverbParams::DRY,
        weight: 0.0,
        zones: 0,
    };
}

impl Default for ReverbState {
    fn default() -> Self {
        Self::DRY
    }
}

/// One zone as the blend reads it: where its centre is, and the zone.
pub type ZoneAt<'a> = (Vec3, &'a ReverbZoneComponent);

/// The listener's weight in a zone at `distance` from its centre: 1 within
/// `min`, 0 at or past `max`, linear between.
pub fn weight(distance: f32, min: f32, max: f32) -> f32 {
    if distance <= min {
        1.0
    } else if distance >= max {
        0.0
    } else {
        (max - distance) / (max - min)
    }
}

/// The reverb a listener at `listener` hears among `zones`. Zones are summed in
/// the order given, so a caller that wants bit-identical runs passes a stable order.
pub fn blend(listener: Vec3, zones: &[ZoneAt]) -> ReverbState {
    let mut total = 0.0;
    let mut count = 0;
    let mut sum = [0.0_f32; 4];
    for (centre, zone) in zones {
        let w = weight(
            listener.distance(*centre),
            zone.min_distance,
            zone.max_distance,
        );
        if w <= 0.0 {
            continue;
        }
        let p = zone.params;
        total += w;
        count += 1;
        for (s, v) in sum
            .iter_mut()
            .zip([p.decay_time, p.pre_delay, p.damping, p.wet])
        {
            *s += w * v;
        }
    }
    if count == 0 {
        return ReverbState::DRY;
    }
    let [decay_time, pre_delay, damping, wet] = sum;
    let params = ReverbParams {
        decay_time: decay_time / total,
        pre_delay: pre_delay / total,
        damping: damping / total,
        wet: wet / total.max(1.0),
    };
    ReverbState {
        params: params.clamped(),
        weight: total.min(1.0),
        zones: count,
    }
}

#[cfg(test)]
#[path = "reverb_tests.rs"]
mod reverb_tests;
