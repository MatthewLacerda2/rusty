use glam::{Quat, Vec3};

use super::introspection::SpatialResult;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Listener {
    pub position: Vec3,
    pub right: Vec3,
}

impl Listener {
    pub fn new(position: Vec3, right: Vec3) -> Self {
        let right = right.try_normalize().unwrap_or(Vec3::X);
        Self { position, right }
    }

    pub fn from_transform(position: Vec3, rotation: Quat) -> Self {
        Self::new(position, rotation * Vec3::X)
    }
}

/// Linear distance rolloff in `[0, 1]`: flat `1.0` out to `initial`, a linear ramp
/// down across `initial → final`, then `0.0` beyond. A non-positive band
/// (`final <= initial`) degenerates to a hard cutoff at `initial`.
fn distance_rolloff(distance: f32, initial: f32, final_dist: f32) -> f32 {
    if distance <= initial {
        return 1.0;
    }
    if distance >= final_dist {
        return 0.0;
    }
    let band = final_dist - initial;
    if band <= 0.0 {
        return 0.0;
    }
    1.0 - (distance - initial) / band
}

pub fn resolve(
    listener: &Listener,
    source_pos: Vec3,
    volume: f32,
    spatial_blend: f32,
    initial_distance: f32,
    final_distance: f32,
) -> SpatialResult {
    let volume = volume.max(0.0);
    let blend = spatial_blend.clamp(0.0, 1.0);

    let to_source = source_pos - listener.position;
    let distance = to_source.length();

    let rolloff = distance_rolloff(distance, initial_distance, final_distance);
    let spatial_gain = volume * rolloff;
    let pan = match to_source.try_normalize() {
        None => 0.0,
        Some(dir) => listener.right.dot(dir).clamp(-1.0, 1.0),
    };

    let gain = volume + (spatial_gain - volume) * blend;
    let pan = pan * blend;

    SpatialResult { gain, pan }
}

#[cfg(test)]
#[path = "spatial_tests.rs"]
mod spatial_tests;
