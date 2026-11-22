mod maestro;

use glam::Vec3;
use serde::{Deserialize, Serialize};

use rusty_components::{ReverbParams, ReverbZoneComponent};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReverbState {
    pub params: ReverbParams,
    pub weight: f32,
    pub zones: usize,
}

impl ReverbState {
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

pub type ZoneAt<'a> = (Vec3, &'a ReverbZoneComponent);

pub fn weight(distance: f32, min: f32, max: f32) -> f32 {
    if distance <= min {
        1.0
    } else if distance >= max {
        0.0
    } else {
        (max - distance) / (max - min)
    }
}

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
