//! src/audio/spatial.rs — 3D audio spatialization math (#213).
//!
//! Pure, device-free spatial resolution: given the **listener** (the active camera)
//! and a source's world position + its `AudioSource` rolloff fields, compute the
//! per-source **`(gain, pan)`** the mixer applies. There is no audio device here and
//! no wall clock or RNG — this is sim-side math, so it is unit-tested headlessly and
//! the `NullBackend` resolves it identically to the real device. Only the *final mix*
//! (handing `(gain, pan)` to the backend) happens in the platform layer.
//!
//! The model (Unity's `AudioSource`, deliberately the linear subset #213 ships):
//!   * **Distance rolloff** — full volume (no attenuation) out to `initial_distance`,
//!     then a LINEAR falloff from `initial_distance → final_distance`, silent beyond.
//!   * **Pan** — the source direction projected onto the listener's right axis, so a
//!     source to the listener's left pans left (`-1`) and to the right pans right
//!     (`+1`); dead ahead/behind is centred (`0`).
//!   * **`spatial_blend`** — linearly lerps 2D ↔ 3D: at `0` the source is pure 2D
//!     (full `volume`, centred — the #212 behaviour), at `1` it is fully spatialized,
//!     and in between each of gain and pan lerps toward the spatial value.
//!
//! Out of scope (same exclusions as the issue): reverb, occlusion, doppler, mixer
//! groups, non-linear rolloff curves, surround/HRTF, pitch.

use glam::{Quat, Vec3};

use super::introspection::SpatialResult;

/// The audio listener — the active camera, reduced to just what spatialization needs:
/// a world position and the unit **right** axis (used to derive left/right pan). The
/// engine is right-handed, Y-up; a transform's right is `rotation * +X`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Listener {
    pub position: Vec3,
    /// Unit vector pointing to the listener's right in world space.
    pub right: Vec3,
}

impl Listener {
    /// Build a listener from a world position and an already-computed right axis (e.g.
    /// the active `render::Camera`'s `position` + `right()`). The right vector is
    /// normalized (falling back to world +X if degenerate).
    pub fn new(position: Vec3, right: Vec3) -> Self {
        let right = right.try_normalize().unwrap_or(Vec3::X);
        Self { position, right }
    }

    /// Build a listener from a camera entity's world transform (position + rotation).
    /// The right axis is `rotation * +X`.
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

/// Resolve a source to its `(gain, pan)` for this listener.
///
/// `volume` is the source's per-source linear gain (pre-master). `spatial_blend`,
/// `initial_distance` and `final_distance` are the `AudioSource` spatial fields. The
/// result is pre-master: the maestro/backend still folds in the master volume.
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

    // 3D endpoint: gain attenuated by distance rolloff, pan from the right-projection.
    let rolloff = distance_rolloff(distance, initial_distance, final_distance);
    let spatial_gain = volume * rolloff;
    let pan = match to_source.try_normalize() {
        // Co-located with the listener: no meaningful direction, so centre it.
        None => 0.0,
        Some(dir) => listener.right.dot(dir).clamp(-1.0, 1.0),
    };

    // 2D endpoint: full volume, centred (the #212 behaviour). Lerp each axis.
    let gain = volume + (spatial_gain - volume) * blend;
    let pan = pan * blend;

    SpatialResult { gain, pan }
}

#[cfg(test)]
#[path = "spatial_tests.rs"]
mod spatial_tests;
