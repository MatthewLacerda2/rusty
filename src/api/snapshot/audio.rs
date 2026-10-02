//! src/api/snapshot/audio.rs — the audio components' `Debug.Snapshot` views: the
//! AudioSource (#212) and the AudioReverbZone (#469).

use serde_json::{json, Value};

use crate::components::{AudioSourceComponent, ReverbZoneComponent};

/// AudioSource authoring view (#212): the clip + playback flags, plus the spatial
/// fields stored now for #213. The live playing state lives in the `AudioMaestro`
/// roster, not here — this is the persistent component's own data.
pub(crate) fn audio_value(a: &AudioSourceComponent) -> Value {
    json!({
        "clip": a.clip,
        "volume": a.volume,
        "loop": a.looping,
        "play_on_start": a.play_on_start,
        "is_time_scaled": a.is_time_scaled,
        "spatial_blend": a.spatial_blend,
        "initial_distance": a.initial_distance,
        "final_distance": a.final_distance,
        "output_group": a.output_group,
        "occlusion_enabled": a.occlusion_enabled,
    })
}

/// AudioReverbZone (#469): its radii, preset and params.
pub(crate) fn reverb_zone_value(z: &ReverbZoneComponent) -> Value {
    let p = z.params;
    json!({
        "min_distance": z.min_distance,
        "max_distance": z.max_distance,
        "preset": z.preset.name(),
        "decay_time": p.decay_time,
        "pre_delay": p.pre_delay,
        "damping": p.damping,
        "wet": p.wet,
    })
}
